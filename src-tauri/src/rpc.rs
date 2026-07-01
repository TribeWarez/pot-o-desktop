use reqwest::Client;
use serde_json::Value;

pub struct PotRpc {
    client: Client,
    base_url: String,
}

impl PotRpc {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn set_base_url(&mut self, url: &str) {
        self.base_url = url.trim_end_matches('/').to_string();
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        resp.json()
            .await
            .map_err(|e| format!("JSON parse failed: {}", e))
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        resp.json()
            .await
            .map_err(|e| format!("JSON parse failed: {}", e))
    }

    /// Submit proof with device info matching the validator's v0.7.3 submit format.
    #[allow(dead_code)]
    pub async fn submit_proof(
        &self,
        proof: Value,
        device_id: Option<String>,
        device_type: Option<String>,
    ) -> Result<Value, String> {
        let mut body = serde_json::json!({
            "proof": proof,
        });
        if let Some(did) = device_id {
            body["device_id"] = Value::String(did);
        }
        if let Some(dt) = device_type {
            body["device_type"] = Value::String(dt);
        }
        self.post("/submit", body).await
    }

    /// Register a device with the validator.
    pub async fn register_device(
        &self,
        device_type: &str,
        device_id: Option<String>,
        miner_pubkey: Option<String>,
    ) -> Result<Value, String> {
        let mut body = serde_json::json!({
            "device_type": device_type,
        });
        if let Some(did) = device_id {
            body["device_id"] = Value::String(did);
        }
        if let Some(pk) = miner_pubkey {
            body["miner_pubkey"] = Value::String(pk);
        }
        self.post("/devices/register", body).await
    }
}
