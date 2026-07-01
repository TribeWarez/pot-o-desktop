use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotOConfig {
    // Connection
    pub rpc_url: String,
    pub status_url: String,
    pub solana_rpc_url: String,

    // Identity
    pub miner_pubkey: String,
    pub miner_json_path: String,
    pub submit_signature: String,

    // Mining parameters
    pub max_iterations: u64,
    pub max_tensor_dim: u64,
    pub loop_delay: u64,
    pub operation: String,
    pub path_layers: String,
    pub mml_threshold: String,

    // Device
    pub device_type: String,
    pub device_id: String,

    // Mode
    pub hexchain_mode: bool,

    // P2P / Network
    pub peer_network_mode: String,
    pub pool_strategy: String,
    pub bootstrap_urls: Vec<String>,
    pub enable_mdns: bool,
    pub mdns_service_name: String,
    pub peer_timeout_secs: u64,
    pub challenge_relay_enabled: bool,

    // Wallet
    pub wallet_base_url: String,
    pub wallet_address: String,

    // Debug
    pub explain: bool,
    pub verbose: bool,
}

impl Default for PotOConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://pot.rpc.gateway.tribewarez.com".into(),
            status_url: "https://status.rpc.gateway.tribewarez.com".into(),
            solana_rpc_url: String::new(),
            miner_pubkey: String::new(),
            miner_json_path: String::new(),
            submit_signature: String::new(),
            max_iterations: 10000,
            max_tensor_dim: 256,
            loop_delay: 2,
            operation: String::new(),
            path_layers: "32,16,8".into(),
            mml_threshold: String::new(),
            device_type: "cpu".into(),
            device_id: String::new(),
            hexchain_mode: false,
            peer_network_mode: "local_only".into(),
            pool_strategy: "solo".into(),
            bootstrap_urls: vec![],
            enable_mdns: false,
            mdns_service_name: "pot-o-desktop".into(),
            peer_timeout_secs: 30,
            challenge_relay_enabled: false,
            wallet_base_url: "https://wallet.rpc.gateway.tribewarez.com".into(),
            wallet_address: String::new(),
            explain: false,
            verbose: false,
        }
    }
}

impl PotOConfig {
    fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pot-o-desktop")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(mut config) = toml::from_str::<Self>(&content) {
                // migrate known outdated URLs
                if config.rpc_url == "https://wallet.rpc.gateway.tribewarez.com/wallet" {
                    config.rpc_url = "https://pot.rpc.gateway.tribewarez.com".into();
                }
                return config;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(Self::config_path(), content).map_err(|e| e.to_string())
    }
}
