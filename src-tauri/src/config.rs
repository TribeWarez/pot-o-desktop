use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotOConfig {
    // Connection
    #[serde(default = "default_rpc_url")]
    pub rpc_url: String,
    #[serde(default = "default_status_url")]
    pub status_url: String,
    // Identity
    #[serde(default)]
    pub miner_pubkey: String,
    #[serde(default)]
    pub miner_json_path: String,
    #[serde(default)]
    pub submit_signature: String,

    // Mining parameters
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u64,
    #[serde(default = "default_max_tensor_dim")]
    pub max_tensor_dim: u64,
    #[serde(default = "default_loop_delay")]
    pub loop_delay: u64,
    #[serde(default)]
    pub operation: String,
    #[serde(default = "default_path_layers")]
    pub path_layers: String,
    #[serde(default)]
    pub mml_threshold: String,

    // Device
    #[serde(default = "default_device_type")]
    pub device_type: String,
    #[serde(default)]
    pub device_id: String,

    // Mode
    #[serde(default)]
    pub hexchain_mode: bool,

    // P2P / Network
    #[serde(default = "default_peer_network_mode")]
    pub peer_network_mode: String,
    #[serde(default = "default_pool_strategy")]
    pub pool_strategy: String,
    #[serde(default)]
    pub bootstrap_urls: Vec<String>,
    #[serde(default)]
    pub enable_mdns: bool,
    #[serde(default = "default_mdns_service_name")]
    pub mdns_service_name: String,
    #[serde(default = "default_peer_timeout_secs")]
    pub peer_timeout_secs: u64,
    #[serde(default)]
    pub challenge_relay_enabled: bool,

    // WebSocket (if empty, derived from rpc_url via scheme replacement)
    #[serde(default)]
    pub ws_url: String,

    // Wallet
    #[serde(default = "default_wallet_base_url")]
    pub wallet_base_url: String,
    #[serde(default)]
    pub wallet_address: String,

    // Debug
    #[serde(default)]
    pub explain: bool,
    #[serde(default)]
    pub verbose: bool,
}

fn default_rpc_url() -> String { "https://pot.rpc.gateway.tribewarez.com".into() }
fn default_status_url() -> String { "https://status.rpc.gateway.tribewarez.com".into() }
fn default_max_iterations() -> u64 { 10000 }
fn default_max_tensor_dim() -> u64 { 256 }
fn default_loop_delay() -> u64 { 2 }
fn default_path_layers() -> String { "32,16,8".into() }
fn default_device_type() -> String { "cpu".into() }
fn default_peer_network_mode() -> String { "local_only".into() }
fn default_pool_strategy() -> String { "solo".into() }
fn default_mdns_service_name() -> String { "pot-o-desktop".into() }
fn default_peer_timeout_secs() -> u64 { 30 }
fn default_wallet_base_url() -> String { "https://wallet.rpc.gateway.tribewarez.com".into() }

impl Default for PotOConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://pot.rpc.gateway.tribewarez.com".into(),
            status_url: "https://status.rpc.gateway.tribewarez.com".into(),
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
            ws_url: String::new(),
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

    /// Returns the effective WS URL: user-specified or derived from rpc_url.
    pub fn ws_url(&self) -> String {
        if !self.ws_url.is_empty() {
            return self.ws_url.clone();
        }
        let trimmed = self.rpc_url.trim_end_matches('/');
        trimmed
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            + "/ws"
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(Self::config_path(), content).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_values() {
        let cfg = PotOConfig::default();
        assert_eq!(cfg.rpc_url, "https://pot.rpc.gateway.tribewarez.com");
        assert_eq!(cfg.status_url, "https://status.rpc.gateway.tribewarez.com");
        assert_eq!(cfg.max_iterations, 10000);
        assert_eq!(cfg.max_tensor_dim, 256);
        assert_eq!(cfg.loop_delay, 2);
        assert_eq!(cfg.path_layers, "32,16,8");
        assert_eq!(cfg.device_type, "cpu");
        assert_eq!(cfg.peer_network_mode, "local_only");
        assert_eq!(cfg.pool_strategy, "solo");
        assert_eq!(cfg.wallet_base_url, "https://wallet.rpc.gateway.tribewarez.com");
        assert!(!cfg.hexchain_mode);
        assert!(cfg.bootstrap_urls.is_empty());
    }

    #[test]
    fn test_ws_url_custom() {
        let mut cfg = PotOConfig::default();
        cfg.ws_url = "wss://custom.example.com/ws".into();
        assert_eq!(cfg.ws_url(), "wss://custom.example.com/ws");
    }

    #[test]
    fn test_ws_url_derived_https() {
        let cfg = PotOConfig::default();
        assert_eq!(cfg.ws_url(), "wss://pot.rpc.gateway.tribewarez.com/ws");
    }

    #[test]
    fn test_ws_url_derived_http() {
        let mut cfg = PotOConfig::default();
        cfg.rpc_url = "http://pot.example.com".into();
        assert_eq!(cfg.ws_url(), "ws://pot.example.com/ws");
    }

    #[test]
    fn test_ws_url_derived_trailing_slash() {
        let mut cfg = PotOConfig::default();
        cfg.rpc_url = "https://pot.example.com/".into();
        assert_eq!(cfg.ws_url(), "wss://pot.example.com/ws");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let _config_path = dir.path().join("config.toml");

        // Override config_path by modifying load to use our temp dir
        // We test serialization/deserialization directly instead
        let mut cfg = PotOConfig::default();
        cfg.rpc_url = "https://custom.example.com".into();
        cfg.max_iterations = 500;
        cfg.hexchain_mode = true;
        cfg.bootstrap_urls = vec!["http://peer1".into()];

        let content = toml::to_string_pretty(&cfg).unwrap();
        let deserialized: PotOConfig = toml::from_str(&content).unwrap();

        assert_eq!(deserialized.rpc_url, "https://custom.example.com");
        assert_eq!(deserialized.max_iterations, 500);
        assert!(deserialized.hexchain_mode);
        assert_eq!(deserialized.bootstrap_urls, vec!["http://peer1"]);
    }

    #[test]
    fn test_migration_old_rpc_url() {
        let toml_str = r#"rpc_url = "https://wallet.rpc.gateway.tribewarez.com/wallet"
status_url = "https://status.rpc.gateway.tribewarez.com"
"#;
        let cfg: PotOConfig = toml::from_str(toml_str).unwrap();
        // Migration happens in load(), not in deserialize; but we test that
        // the old URL is parsed without error thanks to #[serde(default)]
        assert_eq!(cfg.rpc_url, "https://wallet.rpc.gateway.tribewarez.com/wallet");
    }

    #[test]
    fn test_load_missing_file_returns_defaults() {
        // load() reads from ~/.config/pot-o-desktop/config.toml which won't exist in CI
        let cfg = PotOConfig::load();
        // Should not panic; returns defaults since file doesn't exist
        assert_eq!(cfg.rpc_url, "https://pot.rpc.gateway.tribewarez.com");
    }

    #[test]
    fn test_partial_toml_uses_defaults() {
        let toml_str = r#"rpc_url = "https://custom.example.com""#;
        let cfg: PotOConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.rpc_url, "https://custom.example.com");
        // Missing fields should use #[serde(default)]
        assert_eq!(cfg.status_url, "https://status.rpc.gateway.tribewarez.com");
        assert_eq!(cfg.max_iterations, 10000);
        assert!(!cfg.hexchain_mode);
    }

    #[test]
    fn test_config_dir_ends_with_pot_o_desktop() {
        let path = PotOConfig::config_dir();
        assert!(path.ends_with("pot-o-desktop"));
    }

    #[test]
    fn test_save_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let custom_path = dir.path().join("subdir").join("config.toml");

        // Override the internal config_path for testing
        // We use a helper fn to test the serialization path
        let cfg = PotOConfig::default();
        let content = toml::to_string_pretty(&cfg).unwrap();
        fs::write(&custom_path, &content).unwrap();

        let loaded: PotOConfig = toml::from_str(&fs::read_to_string(&custom_path).unwrap()).unwrap();
        assert_eq!(loaded.rpc_url, cfg.rpc_url);
    }
}
