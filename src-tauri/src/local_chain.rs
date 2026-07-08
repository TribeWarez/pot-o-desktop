use hexchain_p2p::block::HexBlock;
use hexchain_p2p::lattice_geometry::HCPCoord;
use serde::{Deserialize, Serialize};

/// Canonical tip tracking — persisted to canonical_tip.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalTip {
    pub height: u64,
    pub block_hash: [u8; 32],
}

impl CanonicalTip {
    pub fn new(height: u64, block_hash: [u8; 32]) -> Self {
        Self { height, block_hash }
    }

    pub fn from_hex(height: u64, block_hash_hex: &str) -> Option<Self> {
        let bytes = hex::decode(block_hash_hex).ok()?;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes[..32.min(bytes.len())]);
        Some(Self {
            height,
            block_hash: hash,
        })
    }

    pub fn block_hash_hex(&self) -> String {
        hex::encode(self.block_hash)
    }
}

/// A transaction entry in the local mempool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTx {
    pub tx_hash: [u8; 32],
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub timestamp: u64,
    pub added_at: std::time::Duration,
}

impl LocalTx {
    pub fn from_json(json: &serde_json::Value) -> Option<Self> {
        let tx_hash = Self::decode_hex(json.get("tx_hash")?.as_str()?, 32)?;
        Some(Self {
            tx_hash,
            from: json.get("from")?.as_str()?.to_string(),
            to: json.get("to")?.as_str()?.to_string(),
            amount: json.get("amount")?.as_str()?.parse().ok()?,
            fee: json.get("fee")?.as_str().unwrap_or("0").parse().ok()?,
            nonce: json.get("nonce")?.as_str().unwrap_or("0").parse().ok()?,
            timestamp: json
                .get("timestamp")?
                .as_str()
                .unwrap_or("0")
                .parse()
                .ok()?,
            added_at: std::time::Duration::from_secs(0),
        })
    }

    fn decode_hex(s: &str, len: usize) -> Option<[u8; 32]> {
        let bytes = hex::decode(s).ok()?;
        let mut arr = [0u8; 32];
        arr[..len.min(bytes.len())].copy_from_slice(&bytes[..len.min(bytes.len())]);
        Some(arr)
    }
}

/// Local mempool — refreshed from validator every 10s.
#[derive(Debug, Clone, Default)]
pub struct LocalMempool {
    pub txs: std::collections::HashMap<[u8; 32], LocalTx>,
}

impl LocalMempool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace mempool with a fresh set of TXs from the validator.
    pub fn refresh(&mut self, validator_txs: Vec<LocalTx>, now_secs: u64) {
        let mut new_txs = std::collections::HashMap::new();
        for tx in validator_txs {
            // Only keep TXs added in the last 30 minutes (1800s)
            if now_secs.saturating_sub(tx.timestamp) < 1800 {
                new_txs.insert(tx.tx_hash, tx);
            }
        }
        *self = Self { txs: new_txs };
    }

    /// Get a Vec of all TXs sorted by fee (desc), then timestamp (asc).
    pub fn sorted_txs(&self) -> Vec<&LocalTx> {
        let mut txs: Vec<_> = self.txs.values().collect();
        txs.sort_by(|a, b| b.fee.cmp(&a.fee).then(a.timestamp.cmp(&b.timestamp)));
        txs
    }

    /// Remove a TX by hash (after it's included in a block).
    pub fn remove(&mut self, tx_hash: &[u8; 32]) {
        self.txs.remove(tx_hash);
    }
}

/// A lightweight block header tracked locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalBlockHeader {
    pub height: u64,
    pub hash: [u8; 32],
    pub coord: HCPCoord,
    pub parent_hash: [u8; 32],
    pub timestamp: u64,
}

impl From<&HexBlock> for LocalBlockHeader {
    fn from(block: &HexBlock) -> Self {
        Self {
            height: block.height,
            hash: block.pow_hash(),
            coord: block.coord,
            parent_hash: block.parent_hash,
            timestamp: block.timestamp,
        }
    }
}
