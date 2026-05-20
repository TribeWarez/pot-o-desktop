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
}
