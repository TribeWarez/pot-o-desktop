use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, watch};
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
    Connected,
    Reconnecting { delay_secs: u64 },
    DashboardUpdate(Value),
}

/// Thread-safe handle for sending messages over the current WS connection.
/// Survives reconnections — sending will fail if WS is permanently disconnected.
#[derive(Clone)]
pub struct WsCmdSender {
    inner: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
}

impl WsCmdSender {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    fn set(&self, tx: mpsc::UnboundedSender<String>) {
        let mut guard = self.inner.blocking_lock();
        *guard = Some(tx);
    }

    fn clear(&self) {
        let mut guard = self.inner.blocking_lock();
        *guard = None;
    }

    pub fn send(&self, msg: String) -> Result<(), String> {
        let guard = self.inner.blocking_lock();
        match guard.as_ref() {
            Some(tx) => tx.send(msg).map_err(|_| "WS channel closed".into()),
            None => Err("WS not connected".into()),
        }
    }
}

pub struct ConnectionHandle {
    pub event_rx: mpsc::UnboundedReceiver<WsEvent>,
    pub cmd_tx: WsCmdSender,
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

    /// Connect and return a handle. The connection is automatically
    /// re-established on drop with exponential backoff (1s → 30s max).
    /// Only a permanent abort (via `abort_rx`) stops reconnection.
    pub async fn connect(
        &self,
        ws_url: &str,
        abort_rx: watch::Receiver<bool>,
    ) -> Result<ConnectionHandle, String> {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<WsEvent>();
        let cmd_sender = WsCmdSender::new();

        let device_id = self.device_id.clone();
        let connected_clone = self.connected.clone();
        let ws_url_owned = ws_url.to_string();
        let cmd_sender_clone = cmd_sender.clone();

        tokio::spawn(async move {
            Self::run_connection_manager(
                &ws_url_owned,
                &device_id,
                connected_clone,
                event_tx,
                cmd_sender_clone,
                abort_rx,
            )
            .await;
        });

        Ok(ConnectionHandle { event_rx, cmd_tx: cmd_sender })
    }

    async fn run_connection_manager(
        ws_url: &str,
        device_id: &str,
        connected: Arc<Mutex<bool>>,
        event_tx: mpsc::UnboundedSender<WsEvent>,
        cmd_sender: WsCmdSender,
        mut abort_rx: watch::Receiver<bool>,
    ) {
        let mut backoff: u64 = 1;

        loop {
            // Check permanent abort before attempting connection
            if *abort_rx.borrow() {
                let _ = event_tx.send(WsEvent::Disconnected);
                break;
            }

            match connect_async(ws_url).await {
                Ok((ws_stream, _)) => {
                    backoff = 1;

                    let subscribe = serde_json::json!({
                        "type": "subscribe",
                        "device_id": device_id,
                        "device_type": "native",
                    });

                    let (mut write, mut read) = ws_stream.split();

                    // Create per-connection channel for outgoing messages
                    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel::<String>();
                    cmd_sender.set(conn_tx);

                    {
                        let mut c = connected.lock().await;
                        *c = true;
                    }
                    let _ = event_tx.send(WsEvent::Connected);

                    // Send subscribe
                    if write.send(Message::Text(subscribe.to_string())).await.is_err() {
                        cmd_sender.clear();
                        {
                            let mut c = connected.lock().await;
                            *c = false;
                        }
                        continue;
                    }

                    // Coordination channels for the inner loops
                    let (conn_abort_tx, conn_abort_rx) = watch::channel(false);
                    let (drop_tx, mut drop_rx) = mpsc::unbounded_channel::<()>();

                    // ── Read loop ─────────────────────────────────
                    let event_tx_read = event_tx.clone();
                    let mut abort_rx_read = abort_rx.clone();
                    let mut conn_abort_rx_read = conn_abort_rx.clone();
                    let drop_tx_read = drop_tx.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(Message::Text(text))) => {
                                            let event = parse_ws_message(&text);
                                            // Don't send Disconnected ourselves;
                                            // the write task sends it when it detects failure.
                                            if !matches!(&event, WsEvent::Disconnected) {
                                                let _ = event_tx_read.send(event);
                                            }
                                        }
                                        Some(Ok(Message::Close(_))) | None => {
                                            let _ = drop_tx_read.send(());
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = abort_rx_read.changed() => break,
                                _ = conn_abort_rx_read.changed() => break,
                            }
                        }
                    });

                    // ── Write + Heartbeat loop ────────────────────
                    let event_tx_write = event_tx.clone();
                    let connected_write = connected.clone();
                    let device_id_write = device_id.to_string();
                    let mut abort_rx_write = abort_rx.clone();
                    let mut conn_abort_rx_write = conn_abort_rx.clone();
                    let drop_tx_write = drop_tx.clone();
                    tokio::spawn(async move {
                        let mut heartbeat_interval =
                            tokio::time::interval(Duration::from_secs(15));
                        heartbeat_interval.reset();

                        loop {
                            tokio::select! {
                                Some(msg) = conn_rx.recv() => {
                                    if write.send(Message::Text(msg)).await.is_err() {
                                        break;
                                    }
                                }
                                _ = heartbeat_interval.tick() => {
                                    if !*connected_write.lock().await { break; }
                                    let heartbeat = serde_json::json!({
                                        "type": "heartbeat",
                                        "device_id": device_id_write,
                                    });
                                    if write.send(Message::Text(heartbeat.to_string())).await.is_err() {
                                        break;
                                    }
                                }
                                _ = abort_rx_write.changed() => break,
                                _ = conn_abort_rx_write.changed() => break,
                            }
                        }

                        let mut c = connected_write.lock().await;
                        let was_connected = *c;
                        *c = false;
                        if was_connected {
                            let _ = event_tx_write.send(WsEvent::Disconnected);
                        }
                        let _ = drop_tx_write.send(());
                    });

                    // ── Wait for drop or abort ────────────────────
                    tokio::select! {
                        _ = drop_rx.recv() => {
                            // Connection dropped — signal inner loops to stop,
                            // clear cmd_sender, then reconnect.
                            let _ = conn_abort_tx.send(true);
                            cmd_sender.clear();
                        }
                        _ = abort_rx.changed() => {
                            // Permanent abort — stop everything.
                            let _ = conn_abort_tx.send(true);
                            cmd_sender.clear();
                            let _ = event_tx.send(WsEvent::Disconnected);
                            break;
                        }
                    }
                }
                Err(_e) => {
                    let _ = event_tx.send(WsEvent::Reconnecting {
                        delay_secs: backoff,
                    });

                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                        _ = abort_rx.changed() => {
                            let _ = event_tx.send(WsEvent::Disconnected);
                            break;
                        }
                    }

                    backoff = std::cmp::min(backoff * 2, 30);
                }
            }
        }
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
            Some("dashboard_update") => WsEvent::DashboardUpdate(v["data"].clone()),
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
