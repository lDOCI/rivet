//! Read-only check against a real Moonraker: connect, identify, subscribe the
//! objects the UI uses, and print what the model makes of the live updates.
//! Sends no commands.
//!
//! `cargo run -p moonraker --example probe -- ws://<printer>:7125/websocket [seconds]`

use std::time::Duration;

use model::{Klippy, PrinterState};
use moonraker::{Client, Event};
use serde_json::Value;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let url = args
        .next()
        .unwrap_or_else(|| "ws://127.0.0.1:7125/websocket".into());
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(15);
    let timeout = Duration::from_secs(5);

    let (client, mut events) = Client::spawn(url.clone());
    let mut state = PrinterState::default();
    let mut updates = 0usize;
    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            ev = events.recv() => match ev {
                Some(Event::Connected) => {
                    println!("connected to {url}");
                    let who = client.identify("Rivet probe", env!("CARGO_PKG_VERSION"), timeout).await?;
                    println!("identify: {who}");
                    let info = client.call_timeout("server.info", Value::Null, timeout).await?;
                    state.klippy = Klippy::parse(info["klippy_state"].as_str().unwrap_or_default());
                    println!("klippy: {:?}, moonraker {}", state.klippy, info["moonraker_version"]);
                    let mut sub = client
                        .call_timeout("printer.objects.subscribe", model::subscription(), timeout)
                        .await?;
                    state.replace_status(sub["status"].take());
                    println!("subscribed: {:#?}", state.snapshot());
                }
                Some(Event::Notification { method, params }) if method == "notify_status_update" => {
                    if let Value::Array(mut args) = params {
                        if !args.is_empty() {
                            state.apply_status(args.swap_remove(0));
                            updates += 1;
                        }
                    }
                }
                Some(Event::Notification { method, .. }) => println!("notification: {method}"),
                Some(Event::Disconnected) => println!("disconnected"),
                Some(Event::Retrying { attempt, next_in }) => println!("attempt {attempt} failed, next in {next_in:?}"),
                None => break,
            },
        }
    }
    println!(
        "{updates} status updates in {secs} s; final: {:#?}",
        state.snapshot()
    );
    Ok(())
}
