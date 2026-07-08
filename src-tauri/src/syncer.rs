use crate::local_chain::{CanonicalTip, LocalMempool, LocalTx};
use crate::storage::{ensure_app_dir, read_json_file, write_json_file, CanonicalTipJson};
use hexchain_p2p::block::HexBlock;
use hexchain_p2p::block_store::BlockStore;
use hexchain_p2p::lattice_store::LatticeStore;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

const BLOCKS_FILE: &str = "hexchain_blocks.json";
const LATTICE_FILE: &str = "hexchain_lattice.json";
const CANONICAL_TIP_FILE: &str = "canonical_tip.json";
const BLOCKS_PERSIST_INTERVAL_SECS: u64 = 30;
const LATTICE_PERSIST_INTERVAL_SECS: u64 = 30;
const MEMPOOL_POLL_INTERVAL_SECS: u64 = 10;

pub struct ChainState {
    pub lattice: Arc<LatticeStore>,
    pub block_store: Arc<BlockStore>,
    pub canonical_tip: Arc<RwLock<CanonicalTip>>,
    pub mempool: Arc<RwLock<LocalMempool>>,
}

impl ChainState {
    pub async fn load_or_init() -> std::io::Result<Self> {
        let app_dir = ensure_app_dir()?;

        let blocks_path = app_dir.join(BLOCKS_FILE);
        let block_store = Arc::new(BlockStore::new(blocks_path.to_string_lossy().as_ref()));

        let lattice_path = app_dir.join(LATTICE_FILE);
        let lattice = Arc::new(LatticeStore::with_path(lattice_path.to_string_lossy().as_ref()));

        let tip_path = app_dir.join(CANONICAL_TIP_FILE);
        let canonical_tip = match read_json_file::<_, CanonicalTipJson>(&tip_path) {
            Some(json) => CanonicalTip::from_hex(json.height, &json.block_hash)
                .unwrap_or_default(),
            None => CanonicalTip::default(),
        };

        Ok(Self {
            lattice,
            block_store,
            canonical_tip: Arc::new(RwLock::new(canonical_tip)),
            mempool: Arc::new(RwLock::new(LocalMempool::new())),
        })
    }

    pub async fn persist_canonical_tip(&self) -> std::io::Result<()> {
        let tip = self.canonical_tip.read().await;
        let app_dir = ensure_app_dir()?;
        let path = app_dir.join(CANONICAL_TIP_FILE);
        let json = CanonicalTipJson {
            height: tip.height,
            block_hash: tip.block_hash_hex(),
        };
        write_json_file(&path, &json)
    }

    pub async fn persist_lattice(&self) -> std::io::Result<()> {
        if let Err(e) = self.lattice.save_to_file() {
            tracing::warn!("Failed to persist lattice: {}", e);
        }
        Ok(())
    }

    pub async fn persist_blocks(&self) -> std::io::Result<()> {
        if let Err(e) = self.block_store.save_to_file() {
            tracing::warn!("Failed to persist blocks: {}", e);
        }
        Ok(())
    }
}

pub struct ChainSyncer {
    rpc_url: String,
    chain_state: Arc<ChainState>,
}

impl ChainSyncer {
    pub fn new(rpc_url: String, chain_state: Arc<ChainState>) -> Self {
        Self { rpc_url, chain_state }
    }

    async fn fetch_validator_tip(&self) -> Result<u64, String> {
        let url = format!("{}/api/blocks?from_height=0&limit=1", self.rpc_url);
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON error: {}", e))?;
        json.get("latest_height")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "missing latest_height".to_string())
    }

    async fn fetch_block(&self, height: u64) -> Result<HexBlock, String> {
        let url = format!("{}/hexchain/block/{}", self.rpc_url, height);
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;
        if resp.status() == 404 {
            return Err("BLOCK_NOT_FOUND".to_string());
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON error: {}", e))?;
        let block: HexBlock = serde_json::from_value(
            json.get("block")
                .ok_or("missing 'block' field")?
                .clone()
        ).map_err(|e| format!("parse error: {}", e))?;
        Ok(block)
    }

    pub async fn sync_once(&self) -> Result<u64, String> {
        let validator_tip = self.fetch_validator_tip().await?;
        let mut our_tip = self.chain_state.canonical_tip.write().await;

        if our_tip.height >= validator_tip {
            return Ok(our_tip.height);
        }

        tracing::info!(from = our_tip.height, to = validator_tip, "Starting catchup");

        let start = our_tip.height + 1;
        let end = validator_tip;
        let mut persisted_count = 0u64;

        for height in start..=end {
            let block = match self.fetch_block(height).await {
                Ok(b) => b,
                Err(ref e) if e == "BLOCK_NOT_FOUND" => {
                    tracing::warn!(height, "Block not found — reached validator's pruned tip");
                    break;
                }
                Err(ref e) => {
                    tracing::warn!(height, error = %e, "Failed to fetch block, retrying next cycle");
                    break;
                }
            };

            let hash = block.pow_hash();
            self.chain_state.lattice.insert(block.coord, hash, block.height);
            let block_json = serde_json::to_string(&block).unwrap_or_default();
            self.chain_state.block_store.insert(&hash, block.height, &block_json);

            our_tip.height = height;
            our_tip.block_hash = hash;

            persisted_count += 1;

            if persisted_count.is_multiple_of(100) {
                drop(our_tip);
                let app_dir = ensure_app_dir().ok();
                if let Some(ref dir) = app_dir {
                    let tip_path = dir.join(CANONICAL_TIP_FILE);
                    let tip = self.chain_state.canonical_tip.read().await;
                    let json = CanonicalTipJson { height: tip.height, block_hash: tip.block_hash_hex() };
                    let _ = write_json_file(&tip_path, &json);
                }
                let _ = self.chain_state.persist_lattice().await;
                let _ = self.chain_state.persist_blocks().await;
                our_tip = self.chain_state.canonical_tip.write().await;
            }
        }

        drop(our_tip);
        let _ = self.chain_state.persist_canonical_tip().await;
        let _ = self.chain_state.persist_lattice().await;
        let _ = self.chain_state.persist_blocks().await;

        let final_tip = self.chain_state.canonical_tip.read().await;
        tracing::info!(synced_to = final_tip.height, "Catchup complete");
        Ok(final_tip.height)
    }

    pub fn spawn_background_sync(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(300));
            loop {
                ticker.tick().await;
                if let Err(e) = self.sync_once().await {
                    tracing::warn!(error = %e, "Background sync failed");
                }
            }
        });
    }
}

pub struct MempoolPoller {
    rpc_url: String,
    chain_state: Arc<ChainState>,
}

impl MempoolPoller {
    pub fn new(rpc_url: String, chain_state: Arc<ChainState>) -> Self {
        Self { rpc_url, chain_state }
    }

    pub async fn poll_once(&self) -> Result<(), String> {
        let url = format!("{}/mempool", self.rpc_url);
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON error: {}", e))?;

        let pending = json.get("pending_transactions").and_then(|v| v.as_array());
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let txs: Vec<LocalTx> = pending
            .into_iter()
            .flatten()
            .filter_map(LocalTx::from_json)
            .collect();

        let mut mempool = self.chain_state.mempool.write().await;
        mempool.refresh(txs, now_secs);

        Ok(())
    }

    pub fn spawn_background_poll(self: Arc<Self>) {
        tokio::spawn(async move {
            if let Err(e) = self.poll_once().await {
                tracing::warn!(error = %e, "Initial mempool poll failed");
            }
            let mut ticker = interval(Duration::from_secs(MEMPOOL_POLL_INTERVAL_SECS));
            loop {
                ticker.tick().await;
                if let Err(e) = self.poll_once().await {
                    tracing::warn!(error = %e, "Mempool poll failed");
                }
            }
        });
    }
}
