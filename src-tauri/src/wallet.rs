use serde_json::Value;

/// Lightweight client for the wallet gateway account management API.
/// Token & marketplace operations go through the existing RPC client.
pub struct WalletClient {
    client: reqwest::Client,
    base_url: String,
}

impl WalletClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .expect("Failed to build reqwest client"),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn list_accounts(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/accounts/list", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let data: Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON parse failed: {}", e))?;
        data["accounts"]
            .as_array()
            .ok_or("Missing accounts field".into())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    }

    pub async fn login(&self, address: &str, password: &str) -> Result<(), String> {
        let url = format!("{}/api/account/login", self.base_url);
        let body = serde_json::json!({ "address": address, "password": password });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Login request failed: {}", e))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let data: Value = resp.json().await.unwrap_or_default();
            let err = data["error"].as_str().unwrap_or("login failed");
            Err(err.to_string())
        }
    }
}
