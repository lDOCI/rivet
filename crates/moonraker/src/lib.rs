//! Minimal async JSON-RPC client for Moonraker's websocket API.
//!
//! One background task owns the socket, reconnects with backoff and routes
//! responses to callers by request id. Server notifications are delivered as
//! [`Event`]s on an unbounded channel.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Default timeout for a JSON-RPC call.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not connected to Moonraker")]
    NotConnected,
    #[error("connection closed before a response arrived")]
    Closed,
    #[error("request timed out")]
    Timeout,
    #[error("Moonraker error {code}: {message}")]
    Rpc { code: i64, message: String },
}

#[derive(Debug, Clone)]
pub enum Event {
    /// Socket is open; previous subscriptions are gone and must be redone.
    Connected,
    /// Socket closed; the client keeps retrying in the background.
    Disconnected,
    /// A connection attempt failed; the next one comes after `next_in`
    /// (or at once after [`Client::retry_now`]). `attempt` counts from 1.
    Retrying { attempt: u32, next_in: Duration },
    /// A server notification such as `notify_status_update`.
    Notification { method: String, params: Value },
}

type Responder = oneshot::Sender<Result<Value, Error>>;
type Pending = Arc<Mutex<HashMap<u64, Responder>>>;

/// Cheap to clone handle for issuing requests.
#[derive(Clone)]
pub struct Client {
    out: mpsc::UnboundedSender<String>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
    connected: Arc<AtomicBool>,
    retry: Arc<tokio::sync::Notify>,
}

impl Client {
    /// Start the connection task. Must be called from inside a tokio runtime.
    pub fn spawn(url: impl Into<String>) -> (Client, mpsc::UnboundedReceiver<Event>) {
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        let client = Client {
            out: out_tx,
            pending: Arc::default(),
            next_id: Arc::new(AtomicU64::new(1)),
            connected: Arc::default(),
            retry: Arc::default(),
        };
        tokio::spawn(run(
            url.into(),
            out_rx,
            ev_tx,
            client.pending.clone(),
            client.connected.clone(),
            client.retry.clone(),
        ));
        (client, ev_rx)
    }

    /// Skip the wait before the next connection attempt.
    pub fn retry_now(&self) {
        self.retry.notify_one();
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, Error> {
        self.call_timeout(method, params, DEFAULT_TIMEOUT).await
    }

    pub async fn call_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, Error> {
        if !self.is_connected() {
            return Err(Error::NotConnected);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut msg = json!({ "jsonrpc": "2.0", "method": method, "id": id });
        if !params.is_null() {
            msg["params"] = params;
        }

        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if self.out.send(msg.to_string()).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err(Error::Closed);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::Closed),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(Error::Timeout)
            }
        }
    }

    /// Run a G-code script. Long moves (homing, probing) can take minutes.
    pub async fn gcode(&self, script: &str) -> Result<(), Error> {
        self.call_timeout(
            "printer.gcode.script",
            json!({ "script": script }),
            Duration::from_secs(600),
        )
        .await
        .map(drop)
    }

    pub async fn identify(
        &self,
        name: &str,
        version: &str,
        timeout: Duration,
    ) -> Result<Value, Error> {
        self.call_timeout(
            "server.connection.identify",
            json!({
                "client_name": name,
                "version": version,
                "type": "display",
                "url": "https://github.com/lDOCI/rivet",
            }),
            timeout,
        )
        .await
    }
}

async fn run(
    url: String,
    mut out_rx: mpsc::UnboundedReceiver<String>,
    ev_tx: mpsc::UnboundedSender<Event>,
    pending: Pending,
    connected: Arc<AtomicBool>,
    retry: Arc<tokio::sync::Notify>,
) {
    const MIN_BACKOFF: Duration = Duration::from_millis(500);
    const MAX_BACKOFF: Duration = Duration::from_secs(5);
    let mut backoff = MIN_BACKOFF;
    let mut attempt = 0u32;

    loop {
        match connect_async(url.as_str()).await {
            Ok((ws, _)) => {
                tracing::info!(%url, "connected to Moonraker");
                backoff = MIN_BACKOFF;
                attempt = 0;
                let (mut sink, mut stream) = ws.split();

                // Requests queued while offline already failed with NotConnected
                // or were failed by `fail_pending`; drop leftovers.
                while out_rx.try_recv().is_ok() {}
                connected.store(true, Ordering::Release);
                if ev_tx.send(Event::Connected).is_err() {
                    return;
                }

                loop {
                    tokio::select! {
                        out = out_rx.recv() => match out {
                            Some(text) => {
                                if let Err(err) = sink.send(Message::text(text)).await {
                                    tracing::warn!(%err, "websocket send failed");
                                    break;
                                }
                            }
                            // Every Client handle is gone: shut down.
                            None => return,
                        },
                        incoming = stream.next() => match incoming {
                            Some(Ok(Message::Text(text))) => {
                                handle_incoming(text.as_str(), &pending, &ev_tx);
                            }
                            Some(Ok(Message::Close(_))) | None => break,
                            Some(Err(err)) => {
                                tracing::warn!(%err, "websocket error");
                                break;
                            }
                            Some(Ok(_)) => {}
                        },
                    }
                }

                connected.store(false, Ordering::Release);
                fail_pending(&pending);
                if ev_tx.send(Event::Disconnected).is_err() {
                    return;
                }
                tracing::info!("disconnected from Moonraker");
            }
            Err(err) => tracing::debug!(%err, "connect failed"),
        }

        if ev_tx.is_closed() {
            return;
        }
        attempt += 1;
        let _ = ev_tx.send(Event::Retrying {
            attempt,
            next_in: backoff,
        });
        tokio::select! {
            () = tokio::time::sleep(backoff) => {}
            () = retry.notified() => {}
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

fn handle_incoming(text: &str, pending: &Pending, ev_tx: &mpsc::UnboundedSender<Event>) {
    let mut msg: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(%err, "invalid JSON from Moonraker");
            return;
        }
    };

    if let Some(id) = msg.get("id").and_then(Value::as_u64) {
        let Some(tx) = pending.lock().unwrap().remove(&id) else {
            return;
        };
        let result = match msg.get("error") {
            Some(err) => Err(Error::Rpc {
                code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
                message: err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }),
            None => Ok(msg
                .get_mut("result")
                .map(Value::take)
                .unwrap_or(Value::Null)),
        };
        let _ = tx.send(result);
    } else if let Some(method) = msg.get("method").and_then(Value::as_str) {
        let method = method.to_owned();
        let params = msg
            .get_mut("params")
            .map(Value::take)
            .unwrap_or(Value::Null);
        let _ = ev_tx.send(Event::Notification { method, params });
    }
}

fn fail_pending(pending: &Pending) {
    for (_, tx) in pending.lock().unwrap().drain() {
        let _ = tx.send(Err(Error::Closed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_response_and_notification() {
        let pending: Pending = Arc::default();
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
        let (tx, mut rx) = oneshot::channel();
        pending.lock().unwrap().insert(7, tx);

        handle_incoming(
            r#"{"jsonrpc":"2.0","result":{"ok":1},"id":7}"#,
            &pending,
            &ev_tx,
        );
        assert_eq!(rx.try_recv().unwrap().unwrap(), json!({ "ok": 1 }));

        handle_incoming(
            r#"{"jsonrpc":"2.0","method":"notify_klippy_ready"}"#,
            &pending,
            &ev_tx,
        );
        match ev_rx.try_recv().unwrap() {
            Event::Notification { method, params } => {
                assert_eq!(method, "notify_klippy_ready");
                assert!(params.is_null());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn routes_rpc_error() {
        let pending: Pending = Arc::default();
        let (ev_tx, _ev_rx) = mpsc::unbounded_channel();
        let (tx, mut rx) = oneshot::channel();
        pending.lock().unwrap().insert(3, tx);

        handle_incoming(
            r#"{"jsonrpc":"2.0","error":{"code":400,"message":"Klippy not ready"},"id":3}"#,
            &pending,
            &ev_tx,
        );
        match rx.try_recv().unwrap() {
            Err(Error::Rpc { code, message }) => {
                assert_eq!(code, 400);
                assert_eq!(message, "Klippy not ready");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
