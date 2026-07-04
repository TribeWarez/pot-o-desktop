use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct PotRpc {
    client: Client,
    base_url: String,
}

impl PotRpc {
    #[allow(dead_code)]
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Failed to build reqwest client"),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn with_client(client: Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let req = match method {
            "GET" => self.client.get(&url),
            "POST" => {
                let b = body.unwrap_or(&serde_json::Value::Null);
                self.client.post(&url).json(b)
            }
            _ => return Err(format!("Unsupported method: {}", method)),
        };
        let timeout_dur = Duration::from_secs(if method == "GET" { 30 } else { 60 });
        let resp = match req.timeout(timeout_dur).send().await {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("{} {} — connection failed: {}", method, url, e);
                crate::logger::error("rpc", &msg);
                return Err(msg);
            }
        };
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            let preview = text.chars().take(300).collect::<String>();
            let msg = format!("{} {} — HTTP {}: {}", method, url, status, preview);
            if status.as_u16() >= 500 {
                crate::logger::warn("rpc", &msg);
            } else {
                crate::logger::error("rpc", &msg);
            }
            return Err(msg);
        }
        match serde_json::from_str::<Value>(&text) {
            Ok(v) => Ok(v),
            Err(e) => {
                let preview = text.chars().take(300).collect::<String>();
                let msg = format!("{} {} — JSON error: {} — body: {}", method, url, e, preview);
                crate::logger::error("rpc", &msg);
                Err(msg)
            }
        }
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        self.request("GET", path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.request("POST", path, Some(&body)).await
    }

    pub async fn register_device(
        &self,
        device_type: &str,
        device_id: Option<String>,
        miner_pubkey: Option<String>,
    ) -> Result<Value, String> {
        let mut body = serde_json::json!({ "device_type": device_type });
        if let Some(did) = device_id {
            body["device_id"] = Value::String(did);
        }
        if let Some(pk) = miner_pubkey {
            body["miner_pubkey"] = Value::String(pk);
        }
        self.post("/devices/register", body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_trims_trailing_slash() {
        let rpc = PotRpc::new("https://example.com/");
        assert_eq!(rpc.base_url, "https://example.com");
    }

    #[test]
    fn test_new_no_trailing_slash() {
        let rpc = PotRpc::new("https://example.com");
        assert_eq!(rpc.base_url, "https://example.com");
    }

    #[test]
    fn test_new_empty_path() {
        let rpc = PotRpc::new("");
        assert_eq!(rpc.base_url, "");
    }
}
