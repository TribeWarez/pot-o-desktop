mod config;
mod keypair;
mod logger;
mod mining;
mod rpc;
mod storage;
mod wallet;
mod local_chain;
mod syncer;
mod ws_client;

use config::PotOConfig;
use mining::MiningEngine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;
use ws_client::WsCmdSender;

fn lock_config<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, PotOConfig>, String> {
    state.config.lock().map_err(|e| format!("Config lock poisoned: {}", e))
}

fn lock_stats<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, MiningStats>, String> {
    state.stats.lock().map_err(|e| format!("Stats lock poisoned: {}", e))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MiningStats {
    pub running: bool,
    pub challenges: u64,
    pub proofs_found: u64,
    pub proofs_submitted: u64,
    pub proofs_accepted: u64,
    pub start_time: u64,
    pub last_challenge_id: String,
    // ── Trace / progress fields (updated live during mining) ──
    pub mining_mode: String,         // "pot_o", "hexchain", or ""
    pub current_nonce: u64,
    pub total_nonces: u64,
    pub best_distance: u32,
    pub current_mml_score: f64,
    pub current_path_sig: String,
    pub current_operation: String,
    pub current_tensor_dims: String, // e.g. "64x64"
    pub hexchain_coord: String,
    pub hexchain_target: String,
}

struct AppState {
    config: Mutex<PotOConfig>,
    engine: Mutex<MiningEngine>,
    stats: Mutex<MiningStats>,
    http_client: reqwest::Client,
    ws_connected: Arc<AtomicBool>,
    ws_abort: Mutex<Option<watch::Sender<bool>>>,
    ws_cmd_tx: Arc<Mutex<Option<WsCmdSender>>>,
    wallet: Mutex<Option<wallet::WalletClient>>,
    wallet_logged_in: AtomicBool,
    chain_state: Arc<syncer::ChainState>,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Result<PotOConfig, String> {
    lock_config(&state).map(|g| g.clone())
}

#[tauri::command]
fn save_config(state: State<AppState>, config: PotOConfig) -> Result<(), String> {
    config.save()?;
    *state.config.lock().map_err(|e| format!("Config lock poisoned: {}", e))? = config;
    Ok(())
}

#[tauri::command]
async fn rpc_get(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    let base_url = {
        let cfg = lock_config(&state)?;
        cfg.rpc_url.clone()
    };
    let rpc = rpc::PotRpc::with_client(state.http_client.clone(), &base_url);
    rpc.get(&path).await
}

#[tauri::command]
async fn rpc_post(
    state: State<'_, AppState>,
    path: String,
    body: Value,
) -> Result<Value, String> {
    let base_url = {
        let cfg = lock_config(&state)?;
        cfg.rpc_url.clone()
    };
    let rpc = rpc::PotRpc::with_client(state.http_client.clone(), &base_url);
    rpc.post(&path, body).await
}

#[tauri::command]
async fn status_api_get(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    let base_url = {
        let cfg = lock_config(&state)?;
        cfg.status_url.clone()
    };
    let rpc = rpc::PotRpc::with_client(state.http_client.clone(), &base_url);
    rpc.get(&path).await
}

#[tauri::command]
fn mine_pot_o(state: State<AppState>, challenge: Value) -> mining::MiningResult {
    let config = match lock_config(&state) {
        Ok(g) => g.clone(),
        Err(_) => return mining::MiningResult {
            status: "no_proof".into(),
            proof: None,
            mml_score: None,
            reason: Some("config_lock_poisoned".into()),
        },
    };
    let engine = state.engine.lock().unwrap_or_else(|e| e.into_inner());
    engine.mine_pot_o(challenge, &config, &state.stats)
}

#[tauri::command]
fn mine_hexchain(state: State<AppState>, challenge: Value) -> mining::MiningResult {
    let config = match lock_config(&state) {
        Ok(g) => g.clone(),
        Err(_) => return mining::MiningResult {
            status: "no_proof".into(),
            proof: None,
            mml_score: None,
            reason: Some("config_lock_poisoned".into()),
        },
    };
    let engine = state.engine.lock().unwrap_or_else(|e| e.into_inner());
    engine.mine_hexchain(challenge, &config, &state.stats, Some(state.chain_state.as_ref()))
}

#[tauri::command]
fn get_mining_stats(state: State<AppState>) -> Result<MiningStats, String> {
    lock_stats(&state).map(|g| g.clone())
}

#[tauri::command]
fn set_mining_stats(state: State<AppState>, stats: MiningStats) -> Result<(), String> {
    *lock_stats(&state)? = stats;
    Ok(())
}

#[tauri::command]
fn start_mining(state: State<AppState>) -> Result<(), String> {
    let mut stats = lock_stats(&state)?;
    stats.running = true;
    stats.start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(())
}

#[tauri::command]
fn stop_mining(state: State<AppState>) -> Result<(), String> {
    let mut stats = lock_stats(&state)?;
    stats.running = false;
    Ok(())
}

#[tauri::command]
fn generate_keypair(path: String) -> Result<keypair::KeypairInfo, String> {
    keypair::generate_keypair(&path)
}

#[tauri::command]
fn read_keypair(path: String) -> Result<keypair::KeypairInfo, String> {
    keypair::pubkey_from_file(&path)
}

#[tauri::command]
fn is_keypair_file(path: String) -> bool {
    keypair::is_full_keypair(&path)
}

#[tauri::command]
async fn register_device(
    state: State<'_, AppState>,
    device_type: String,
    device_id: Option<String>,
    miner_pubkey: Option<String>,
) -> Result<Value, String> {
    let base_url = {
        let cfg = lock_config(&state)?;
        cfg.rpc_url.clone()
    };
    let rpc = rpc::PotRpc::with_client(state.http_client.clone(), &base_url);
    rpc.register_device(&device_type, device_id, miner_pubkey).await
}

#[tauri::command]
async fn ws_connect(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (ws_url, device_id) = {
        let cfg = lock_config(&state)?;
        (cfg.ws_url(), cfg.device_id.clone())
    };
    let did = if device_id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        device_id
    };

    // Create abort signal
    let (abort_tx, abort_rx) = watch::channel(false);

    // Connect — returns ConnectionHandle with event_rx and cmd_tx
    let client = ws_client::WsClient::new(&did);
    let handle = client.connect(&ws_url, abort_rx).await?;

    // Store abort sender so disconnect can signal it
    *state.ws_abort.lock().map_err(|e| format!("WS abort lock poisoned: {}", e))? = Some(abort_tx);
    // Store cmd sender so ws_send command can use it
    *state.ws_cmd_tx.lock().map_err(|e| format!("WS cmd_tx lock poisoned: {}", e))? = Some(handle.cmd_tx.clone());
    state.ws_connected.store(true, Ordering::SeqCst);

    // Spawn task to relay WS events to frontend
    let app_clone = app.clone();
    let connected_flag = state.ws_connected.clone();
    let cmd_tx = state.ws_cmd_tx.clone();
    tokio::spawn(async move {
        let mut rx = handle.event_rx;
        while let Some(event) = rx.recv().await {
            match &event {
                ws_client::WsEvent::Challenge(c) => {
                    let _ = app_clone.emit("ws-challenge", c.clone());
                }
                ws_client::WsEvent::HeartbeatAck => {
                    let _ = app_clone.emit("ws-heartbeat-ack", ());
                }
                ws_client::WsEvent::Connected => {
                    connected_flag.store(true, Ordering::SeqCst);
                    let _ = app_clone.emit("ws-connected", ());
                }
                ws_client::WsEvent::Reconnecting { delay_secs } => {
                    let _ = app_clone.emit("ws-reconnecting", serde_json::json!({ "delay_secs": delay_secs }));
                }
                ws_client::WsEvent::DashboardUpdate(data) => {
                    let _ = app_clone.emit("ws-dashboard-update", data.clone());
                }
                ws_client::WsEvent::Disconnected => {
                    connected_flag.store(false, Ordering::SeqCst);
                    if let Ok(mut guard) = cmd_tx.lock() {
                        *guard = None;
                    }
                    let _ = app_clone.emit("ws-disconnected", ());
                    break;
                }
                ws_client::WsEvent::Subscribed { device_id } => {
                    let _ = app_clone.emit("ws-subscribed", serde_json::json!({ "device_id": device_id }));
                }
                ws_client::WsEvent::ProofAccepted { tx_signature } => {
                    let _ = app_clone.emit("ws-proof-accepted", serde_json::json!({ "tx_signature": tx_signature }));
                }
                ws_client::WsEvent::ProofRejected { reason } => {
                    let _ = app_clone.emit("ws-proof-rejected", serde_json::json!({ "reason": reason }));
                }
                ws_client::WsEvent::Error { code, message } => {
                    let _ = app_clone.emit("ws-error", serde_json::json!({ "code": code, "message": message }));
                }
            }
        }
        connected_flag.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = cmd_tx.lock() {
            *guard = None;
        }
        let _ = app_clone.emit("ws-disconnected", ());
    });

    Ok(did)
}

#[tauri::command]
async fn ws_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(tx) = state.ws_abort.lock().map_err(|e| format!("WS abort lock poisoned: {}", e))?.take() {
        let _ = tx.send(true);
    }
    *state.ws_cmd_tx.lock().map_err(|e| format!("WS cmd_tx lock poisoned: {}", e))? = None;
    state.ws_connected.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn ws_is_connected(state: State<AppState>) -> bool {
    state.ws_connected.load(Ordering::SeqCst)
}

#[tauri::command]
async fn ws_send(state: State<'_, AppState>, payload: Value) -> Result<(), String> {
    let msg = serde_json::to_string(&payload).map_err(|e| format!("WS serialize error: {}", e))?;
    let cmd_tx = state.ws_cmd_tx.lock().map_err(|e| format!("WS cmd_tx lock poisoned: {}", e))?;
    match cmd_tx.as_ref() {
        Some(sender) => sender.send(msg),
        None => Err("WS not connected".into()),
    }
}

#[tauri::command]
async fn wallet_list_accounts(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    // Take the wallet client out of state to avoid holding MutexGuard across await.
    let client = state.wallet.lock()
        .map_err(|e| format!("Wallet lock poisoned: {}", e))?
        .take();

    match client {
        Some(c) => {
            let result = c.list_accounts().await;
            // Put the client back if still logged in
            if result.is_ok() && state.wallet_logged_in.load(Ordering::SeqCst) {
                let _ = state.wallet.lock().map_err(|e| format!("Wallet lock poisoned: {}", e))?.replace(c);
            }
            result
        }
        None => {
            let base_url = {
                let cfg = lock_config(&state)?;
                cfg.wallet_base_url.clone()
            };
            let client = wallet::WalletClient::new(&base_url);
            client.list_accounts().await
        }
    }
}

#[tauri::command]
async fn wallet_login(
    state: State<'_, AppState>,
    address: String,
    password: String,
) -> Result<(), String> {
    let base_url = {
        let cfg = lock_config(&state)?;
        cfg.wallet_base_url.clone()
    };
    let client = wallet::WalletClient::new(&base_url);
    client.login(&address, &password).await?;
    *state.wallet.lock().map_err(|e| format!("Wallet lock poisoned: {}", e))? = Some(client);
    state.wallet_logged_in.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn wallet_is_logged_in(state: State<AppState>) -> bool {
    state.wallet_logged_in.load(Ordering::SeqCst)
}

#[tauri::command]
fn wallet_logout(state: State<AppState>) -> Result<(), String> {
    *state.wallet.lock().map_err(|e| format!("Wallet lock poisoned: {}", e))? = None;
    state.wallet_logged_in.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn read_log(max_lines: Option<usize>) -> Result<String, String> {
    logger::read_log(max_lines.unwrap_or(100))
}

#[tauri::command]
fn clear_log() -> Result<(), String> {
    logger::clear_log()
}

#[tauri::command]
async fn get_canonical_tip(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let tip = state.chain_state.canonical_tip.read().await;
    Ok(serde_json::json!({
        "height": tip.height,
        "block_hash": tip.block_hash_hex(),
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = PotOConfig::load();

    let chain_state = tauri::async_runtime::block_on(syncer::ChainState::load_or_init())
        .expect("Failed to initialize chain state");

    let chain_state = Arc::new(chain_state);

    {
        let syncer = Arc::new(syncer::ChainSyncer::new(
            config.rpc_url.clone(),
            chain_state.clone(),
        ));
        tauri::async_runtime::block_on(syncer.sync_once());
        syncer.clone().spawn_background_sync();
    }

    {
        let poller = Arc::new(syncer::MempoolPoller::new(
            config.rpc_url.clone(),
            chain_state.clone(),
        ));
        poller.clone().spawn_background_poll();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            config: Mutex::new(config),
            engine: Mutex::new(MiningEngine::new()),
            stats: Mutex::new(MiningStats::default()),
            http_client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("Failed to build shared reqwest client"),
            ws_connected: Arc::new(AtomicBool::new(false)),
            ws_abort: Mutex::new(None),
            ws_cmd_tx: Arc::new(Mutex::new(None)),
            wallet: Mutex::new(None),
            wallet_logged_in: AtomicBool::new(false),
            chain_state,
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            rpc_get,
            rpc_post,
            status_api_get,
            mine_pot_o,
            mine_hexchain,
            get_mining_stats,
            set_mining_stats,
            start_mining,
            stop_mining,
            generate_keypair,
            read_keypair,
            is_keypair_file,
            register_device,
            ws_connect,
            ws_disconnect,
            ws_is_connected,
            ws_send,
            wallet_list_accounts,
            wallet_login,
            wallet_is_logged_in,
            wallet_logout,
            read_log,
            clear_log,
            get_canonical_tip,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
