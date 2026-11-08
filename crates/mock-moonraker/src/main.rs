//! Fake Moonraker for UI work on a laptop.
//!
//! Simulates heaters that approach their targets, a print that advances while
//! running, and Klippy shutdown on emergency stop. Handles just enough of the
//! JSON-RPC API for Rivet.
//!
//! Run: `cargo run -p mock-moonraker` then `cargo run -p rivet`.
//! Commands typed into the mock's terminal: `print [file name]`, `shutdown`, `ready`.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug)]
struct Sim {
    klippy: &'static str,
    state_message: String,
    status: Value,
}

impl Sim {
    fn new() -> Self {
        Self {
            klippy: "ready",
            state_message: "Printer is ready".into(),
            status: json!({
                "webhooks": { "state": "ready", "state_message": "Printer is ready" },
                "print_stats": { "state": "standby", "filename": "", "print_duration": 0.0 },
                "virtual_sdcard": { "progress": 0.0, "is_active": false },
                "display_status": { "progress": 0.0, "message": null },
                "extruder": { "temperature": 24.3, "target": 0.0, "power": 0.0 },
                "heater_bed": { "temperature": 23.8, "target": 0.0, "power": 0.0 },
                "toolhead": { "homed_axes": "", "position": [0.0, 0.0, 0.0, 0.0] },
                "gcode_move": { "speed_factor": 1.0, "extrude_factor": 1.0 },
                "fan": { "speed": 0.0 },
            }),
        }
    }

    fn set(&mut self, delta: Value, out: &mut Value) {
        model::merge(&mut self.status, delta.clone());
        model::merge(out, delta);
    }

    /// Advance the simulation one step, returning the changed fields.
    fn step(&mut self) -> Value {
        let mut delta = json!({});
        if self.klippy != "ready" {
            return delta;
        }
        for heater in ["extruder", "heater_bed"] {
            let t = self.status[heater]["temperature"].as_f64().unwrap_or(0.0);
            let target = self.status[heater]["target"].as_f64().unwrap_or(0.0);
            let goal = if target > 0.0 { target } else { 24.0 };
            let next = t + (goal - t) * 0.08;
            if (next - t).abs() > 0.05 {
                let mut change = json!({});
                change[heater] = json!({ "temperature": (next * 10.0).round() / 10.0 });
                self.set(change, &mut delta);
            }
        }
        if self.status["print_stats"]["state"] == "printing" {
            let p = self.status["virtual_sdcard"]["progress"]
                .as_f64()
                .unwrap_or(0.0)
                + 0.002;
            if p >= 1.0 {
                self.set(
                    json!({
                        "print_stats": { "state": "complete" },
                        "virtual_sdcard": { "progress": 1.0, "is_active": false },
                    }),
                    &mut delta,
                );
            } else {
                self.set(json!({ "virtual_sdcard": { "progress": p } }), &mut delta);
            }
        }
        delta
    }

    fn gcode(&mut self, script: &str, delta: &mut Value) {
        for line in script.lines() {
            let mut words = line.split_whitespace();
            let cmd = words.next().unwrap_or_default().to_ascii_uppercase();
            let s = words
                .find_map(|w| w.strip_prefix(['S', 's']))
                .and_then(|v| v.parse::<f64>().ok());
            match (cmd.as_str(), s) {
                ("M104" | "M109", Some(t)) => {
                    self.set(json!({ "extruder": { "target": t } }), delta)
                }
                ("M140" | "M190", Some(t)) => {
                    self.set(json!({ "heater_bed": { "target": t } }), delta)
                }
                ("TURN_OFF_HEATERS", _) => self.set(
                    json!({ "extruder": { "target": 0.0 }, "heater_bed": { "target": 0.0 } }),
                    delta,
                ),
                ("G28", _) => self.set(json!({ "toolhead": { "homed_axes": "xyz" } }), delta),
                _ => {}
            }
        }
    }

    fn start_print(&mut self, filename: &str, delta: &mut Value) {
        self.set(
            json!({
                "print_stats": { "state": "printing", "filename": filename },
                "virtual_sdcard": { "progress": 0.0, "is_active": true },
                "extruder": { "target": 210.0 },
                "heater_bed": { "target": 60.0 },
            }),
            delta,
        );
    }

    fn shutdown(&mut self, reason: &str) {
        self.klippy = "shutdown";
        self.state_message = reason.into();
        self.status["webhooks"] = json!({ "state": "shutdown", "state_message": reason });
        self.status["print_stats"]["state"] = json!("error");
        for heater in ["extruder", "heater_bed"] {
            self.status[heater]["target"] = json!(0.0);
        }
    }

    fn make_ready(&mut self) {
        self.klippy = "ready";
        self.state_message = "Printer is ready".into();
        self.status["webhooks"] = json!({ "state": "ready", "state_message": "Printer is ready" });
        self.status["print_stats"] =
            json!({ "state": "standby", "filename": "", "print_duration": 0.0 });
        self.status["virtual_sdcard"] = json!({ "progress": 0.0, "is_active": false });
    }
}

type Shared = Arc<Mutex<Sim>>;

/// Broadcast messages to every connected client.
#[derive(Clone, Debug)]
enum Push {
    Status(Value),
    Notify(&'static str),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let addr: SocketAddr = std::env::var("MOCK_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7125".into())
        .parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(
        "mock Moonraker on ws://{addr}/websocket (type: print [file] | shutdown | ready)"
    );

    let sim: Shared = Arc::new(Mutex::new(Sim::new()));
    let (push_tx, _) = broadcast::channel::<Push>(64);

    // Physics tick, 4 Hz like Klipper's status updates.
    {
        let sim = sim.clone();
        let push_tx = push_tx.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                tick.tick().await;
                let delta = sim.lock().unwrap().step();
                if delta.as_object().is_some_and(|o| !o.is_empty()) {
                    let _ = push_tx.send(Push::Status(delta));
                }
            }
        });
    }

    // Terminal control for scenarios the UI can't trigger yet.
    {
        let sim = sim.clone();
        let push_tx = push_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut sim = sim.lock().unwrap();
                let line = line.trim();
                let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
                match cmd {
                    "print" => {
                        let filename = match arg.trim() {
                            "" => "benchy_0.2mm_PLA_AD5M.gcode",
                            name => name,
                        };
                        let mut delta = json!({});
                        sim.start_print(filename, &mut delta);
                        let _ = push_tx.send(Push::Status(delta));
                    }
                    "shutdown" => {
                        sim.shutdown("MCU 'mcu' shutdown: Timer too close");
                        let _ = push_tx.send(Push::Notify("notify_klippy_shutdown"));
                    }
                    "ready" => {
                        sim.make_ready();
                        let _ = push_tx.send(Push::Notify("notify_klippy_ready"));
                    }
                    other => tracing::warn!("unknown command {other:?}"),
                }
            }
        });
    }

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(serve(stream, peer, sim.clone(), push_tx.clone()));
    }
}

async fn serve(stream: TcpStream, peer: SocketAddr, sim: Shared, push_tx: broadcast::Sender<Push>) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(err) => {
            tracing::warn!(%peer, %err, "handshake failed");
            return;
        }
    };
    tracing::info!(%peer, "client connected");
    let (mut sink, mut stream) = ws.split();
    let mut pushes = push_tx.subscribe();

    loop {
        tokio::select! {
            msg = stream.next() => {
                let text = match msg {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(_)) => continue,
                    Some(Err(_)) | None => break,
                };
                let Ok(req) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                let (reply, broadcast) = handle(&sim, &req);
                if let Some(reply) = reply {
                    if sink.send(Message::text(reply.to_string())).await.is_err() {
                        break;
                    }
                }
                if let Some(push) = broadcast {
                    let _ = push_tx.send(push);
                }
            }
            push = pushes.recv() => {
                let msg = match push {
                    Ok(Push::Status(delta)) => json!({
                        "jsonrpc": "2.0",
                        "method": "notify_status_update",
                        "params": [delta, 0.0],
                    }),
                    Ok(Push::Notify(method)) => json!({ "jsonrpc": "2.0", "method": method }),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if sink.send(Message::text(msg.to_string())).await.is_err() {
                    break;
                }
            }
        }
    }
    tracing::info!(%peer, "client disconnected");
}

fn handle(sim: &Shared, req: &Value) -> (Option<Value>, Option<Push>) {
    let id = req.get("id").cloned();
    let method = req["method"].as_str().unwrap_or_default();
    let params = &req["params"];
    let mut sim = sim.lock().unwrap();
    let mut delta = json!({});
    let mut push = None;

    const NOT_READY: (i64, &str) = (503, "Klippy Host not connected");
    let result: Result<Value, (i64, &str)> = match method {
        "server.connection.identify" => Ok(json!({ "connection_id": 1 })),
        "server.info" => Ok(json!({ "klippy_connected": true, "klippy_state": sim.klippy })),
        "printer.info" => Ok(json!({ "state": sim.klippy, "state_message": sim.state_message })),
        "printer.objects.subscribe" | "printer.objects.query" => {
            if sim.klippy != "ready" {
                Err(NOT_READY)
            } else {
                let mut status = json!({});
                if let Some(objects) = params["objects"].as_object() {
                    for name in objects.keys() {
                        status[name] = sim.status.get(name).cloned().unwrap_or(json!({}));
                    }
                }
                Ok(json!({ "eventtime": 0.0, "status": status }))
            }
        }
        "printer.gcode.script" if sim.klippy == "ready" => {
            sim.gcode(params["script"].as_str().unwrap_or_default(), &mut delta);
            Ok(json!("ok"))
        }
        "printer.print.pause" => {
            sim.set(json!({ "print_stats": { "state": "paused" } }), &mut delta);
            Ok(json!("ok"))
        }
        "printer.print.resume" => {
            sim.set(
                json!({ "print_stats": { "state": "printing" } }),
                &mut delta,
            );
            Ok(json!("ok"))
        }
        "printer.print.cancel" => {
            sim.set(
                json!({
                    "print_stats": { "state": "cancelled" },
                    "virtual_sdcard": { "is_active": false },
                    "extruder": { "target": 0.0 },
                    "heater_bed": { "target": 0.0 },
                }),
                &mut delta,
            );
            Ok(json!("ok"))
        }
        "printer.emergency_stop" => {
            sim.shutdown("Shutdown due to webhooks request");
            push = Some(Push::Notify("notify_klippy_shutdown"));
            Ok(json!("ok"))
        }
        "printer.firmware_restart" | "printer.restart" => {
            sim.make_ready();
            push = Some(Push::Notify("notify_klippy_ready"));
            Ok(json!("ok"))
        }
        m if m.starts_with("printer.") => Err(NOT_READY),
        _ => Err((404, "Method not found")),
    };

    if push.is_none() && delta.as_object().is_some_and(|o| !o.is_empty()) {
        push = Some(Push::Status(delta));
    }

    let reply = id.map(|id| match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "result": result, "id": id }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "error": { "code": code, "message": message },
            "id": id,
        }),
    });
    (reply, push)
}
