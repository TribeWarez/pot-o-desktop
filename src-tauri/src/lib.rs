mod config;
mod keypair;
mod logger;
mod mining;
mod rpc;
mod wallet;
mod ws_client;

use config::PotOConfig;
use mining::MiningEngine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningStats {
    pub running: bool,
    pub challenges: u64,
    pub proofs_found: u64,
    pub proofs_submitted: u64,
    pub proofs_accepted: u64,
    pub start_time: u64,
    pub last_challenge_id: String,
}

impl Default for MiningStats {
    fn default() -> Self {
        Self {
            running: false,
            challenges: 0,
            proofs_found: 0,
            proofs_submitted: 0,
            proofs_accepted: 0,
            start_time: 0,
            last_challenge_id: String::new(),
        }
    }
}

struct AppState {
    config: Mutex<PotOConfig>,
    engine: Mutex<MiningEngine>,
    stats: Mutex<MiningStats>,
    ws_connected: AtomicBool,
    wallet: Mutex<Option<wallet::WalletClient>>,
    wallet_logged_in: AtomicBool,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> PotOConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(state: State<AppState>, config: PotOConfig) -> Result<(), String> {
    config.save()?;
    *state.config.lock().unwrap() = config;
    Ok(())
}

#[tauri::command]
async fn rpc_get(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    let base_url = {
        let cfg = state.config.lock().unwrap();
        cfg.rpc_url.clone()
    };
    let rpc = rpc::PotRpc::new(&base_url);
    rpc.get(&path).await
}

#[tauri::command]
async fn rpc_post(
    state: State<'_, AppState>,
    path: String,
    body: Value,
) -> Result<Value, String> {
    let base_url = {
        let cfg = state.config.lock().unwrap();
        cfg.rpc_url.clone()
    };
    let rpc = rpc::PotRpc::new(&base_url);
    rpc.post(&path, body).await
}

#[tauri::command]
async fn status_api_get(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    let base_url = {
        let cfg = state.config.lock().unwrap();
        cfg.status_url.clone()
    };
    let rpc = rpc::PotRpc::new(&base_url);
    rpc.get(&path).await
}

#[tauri::command]
fn mine_pot_o(state: State<AppState>, challenge: Value) -> mining::MiningResult {
    let config = state.config.lock().unwrap().clone();
    let engine = state.engine.lock().unwrap();
    engine.mine_pot_o(challenge, &config)
}

#[tauri::command]
fn mine_hexchain(state: State<AppState>, challenge: Value) -> mining::MiningResult {
    let config = state.config.lock().unwrap().clone();
    let engine = state.engine.lock().unwrap();
    engine.mine_hexchain(challenge, &config)
}

#[tauri::command]
fn get_mining_stats(state: State<AppState>) -> MiningStats {
    state.stats.lock().unwrap().clone()
}

#[tauri::command]
fn set_mining_stats(state: State<AppState>, stats: MiningStats) {
    *state.stats.lock().unwrap() = stats;
}

#[tauri::command]
fn start_mining(state: State<AppState>) {
    let mut stats = state.stats.lock().unwrap();
    stats.running = true;
    stats.start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
}

#[tauri::command]
fn stop_mining(state: State<AppState>) {
    let mut stats = state.stats.lock().unwrap();
    stats.running = false;
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
    keypair::is_solana_keypair(&path)
}

#[tauri::command]
async fn register_device(
    state: State<'_, AppState>,
    device_type: String,
    device_id: Option<String>,
    miner_pubkey: Option<String>,
) -> Result<Value, String> {
    let base_url = {
        let cfg = state.config.lock().unwrap();
        cfg.rpc_url.clone()
    };
    let rpc = rpc::PotRpc::new(&base_url);
    rpc.register_device(&device_type, device_id, miner_pubkey).await
}

#[tauri::command]
async fn ws_connect(state: State<'_, AppState>) -> Result<String, String> {
    let (base_url, device_id) = {
        let cfg = state.config.lock().unwrap();
        (cfg.rpc_url.clone(), cfg.device_id.clone())
    };
    let did = if device_id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        device_id
    };

    let client = ws_client::WsClient::new(&did, &base_url);
    let _rx = client.connect().await?;

    state.ws_connected.store(true, Ordering::SeqCst);

    Ok(did)
}

#[tauri::command]
async fn ws_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    state.ws_connected.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn ws_is_connected(state: State<AppState>) -> bool {
    state.ws_connected.load(Ordering::SeqCst)
}

#[tauri::command]
async fn wallet_list_accounts(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let base_url = {
        let cfg = state.config.lock().unwrap();
        cfg.wallet_base_url.clone()
    };
    let client = wallet::WalletClient::new(&base_url);
    client.list_accounts().await
}

#[tauri::command]
async fn wallet_login(
    state: State<'_, AppState>,
    address: String,
    password: String,
) -> Result<(), String> {
    let base_url = {
        let cfg = state.config.lock().unwrap();
        cfg.wallet_base_url.clone()
    };
    let client = wallet::WalletClient::new(&base_url);
    client.login(&address, &password).await?;
    *state.wallet.lock().unwrap() = Some(client);
    state.wallet_logged_in.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn wallet_is_logged_in(state: State<AppState>) -> bool {
    state.wallet_logged_in.load(Ordering::SeqCst)
}

#[tauri::command]
fn wallet_logout(state: State<AppState>) {
    *state.wallet.lock().unwrap() = None;
    state.wallet_logged_in.store(false, Ordering::SeqCst);
}

#[tauri::command]
fn read_log(max_lines: Option<usize>) -> Result<String, String> {
    logger::read_log(max_lines.unwrap_or(100))
}

#[tauri::command]
fn clear_log() -> Result<(), String> {
    logger::clear_log()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = PotOConfig::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            config: Mutex::new(config),
            engine: Mutex::new(MiningEngine::new()),
            stats: Mutex::new(MiningStats::default()),
            ws_connected: AtomicBool::new(false),
            wallet: Mutex::new(None),
            wallet_logged_in: AtomicBool::new(false),
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
            wallet_list_accounts,
            wallet_login,
            wallet_is_logged_in,
            wallet_logout,
            read_log,
            clear_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
