use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[allow(dead_code)]
pub enum WsEvent {
    Challenge(Value),
    ProofAccepted { tx_signature: String },
    ProofRejected { reason: String },
    HeartbeatAck,
    Subscribed { device_id: String },
    Error { code: String, message: String },
    Disconnected,
}

pub struct WsClient {
    pub device_id: String,
    connected: Arc<Mutex<bool>>,
}

impl WsClient {
    pub fn new(device_id: &str) -> Self {
        Self {
            device_id: device_id.to_string(),
            connected: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn is_connected(&self) -> bool {
        *self.connected.lock().await
    }

    pub async fn connect(
        &self,
        ws_url: &str,
        mut abort_rx: watch::Receiver<bool>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<WsEvent>, String> {
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| format!("WS connect failed: {}", e))?;

        let (mut write, mut read) = ws_stream.split();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        {
            let mut connected = self.connected.lock().await;
            *connected = true;
        }

        // Send subscribe message
        let subscribe = serde_json::json!({
            "type": "subscribe",
            "device_id": self.device_id,
            "device_type": "native",
        });
        write
            .send(Message::Text(subscribe.to_string()))
            .await
            .map_err(|e| format!("WS subscribe failed: {}", e))?;

        // Spawn read loop
        let tx_clone = tx.clone();
        let connected_clone = self.connected.clone();
        let mut abort = abort_rx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                let event = parse_ws_message(&text);
                                if tx_clone.send(event).is_err() {
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => {
                                let _ = tx_clone.send(WsEvent::Disconnected);
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = abort.changed() => {
                        let _ = tx_clone.send(WsEvent::Disconnected);
                        break;
                    }
                }
            }
            let mut c = connected_clone.lock().await;
            *c = false;
        });

        // Spawn heartbeat task (every 15 seconds)
        let tx_heartbeat = tx.clone();
        let connected_hb = self.connected.clone();
        let device_id_hb = self.device_id.clone();
        let mut abort_hb = abort_rx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {
                        if !*connected_hb.lock().await {
                            break;
                        }
                        let heartbeat = serde_json::json!({
                            "type": "heartbeat",
                            "device_id": device_id_hb,
                        });
                        if write.send(Message::Text(heartbeat.to_string())).await.is_err() {
                            let mut c = connected_hb.lock().await;
                            *c = false;
                            let _ = tx_heartbeat.send(WsEvent::Disconnected);
                            break;
                        }
                    }
                    _ = abort_hb.changed() => {
                        let mut c = connected_hb.lock().await;
                        *c = false;
                        let _ = tx_heartbeat.send(WsEvent::Disconnected);
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}

fn parse_ws_message(text: &str) -> WsEvent {
    match serde_json::from_str::<Value>(text) {
        Ok(v) => match v["type"].as_str() {
            Some("challenge") => {
                if let Some(challenge_str) = v["challenge_json"].as_str() {
                    if let Ok(challenge) = serde_json::from_str::<Value>(challenge_str) {
                        return WsEvent::Challenge(challenge);
                    }
                }
                WsEvent::Error {
                    code: "parse".into(),
                    message: "invalid challenge_json".into(),
                }
            }
            Some("proof_accepted") => WsEvent::ProofAccepted {
                tx_signature: v["tx_signature"].as_str().unwrap_or("").to_string(),
            },
            Some("proof_rejected") => WsEvent::ProofRejected {
                reason: v["reason"].as_str().unwrap_or("unknown").to_string(),
            },
            Some("heartbeat_ack") => WsEvent::HeartbeatAck,
            Some("subscribed") => WsEvent::Subscribed {
                device_id: v["device_id"].as_str().unwrap_or("").to_string(),
            },
            Some("error") => WsEvent::Error {
                code: v["code"].as_str().unwrap_or("unknown").to_string(),
                message: v["message"].as_str().unwrap_or("").to_string(),
            },
            _ => WsEvent::Error {
                code: "unknown_type".into(),
                message: format!("Unknown WS message type: {}", v["type"]),
            },
        },
        Err(e) => WsEvent::Error {
            code: "parse_error".into(),
            message: format!("WS parse error: {}", e),
        },
    }
}
