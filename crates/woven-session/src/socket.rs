use anyhow::Result;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::state::{SessionEvent, SessionState};

pub async fn run_socket_server(state: SessionState, mut shutdown: tokio::sync::mpsc::Receiver<()>) -> Result<()> {
    let sock_path = "/tmp/woven-session.sock";
    let _ = std::fs::remove_file(sock_path);

    let listener = UnixListener::bind(sock_path)?;
    tracing::info!("session listening on {}", sock_path);

    let (tx, _) = broadcast::channel::<SessionEvent>(32);

    loop {
        tokio::select! {
            accept = listener.accept() => {
                let (stream, _) = accept?;
                let state = SessionState {
                    battery: state.battery.clone(),
                    media: state.media.clone(),
                    power: state.power.clone(),
                };
                let tx = tx.clone();
                let rx = tx.subscribe();
                tokio::spawn(async move {
                    let _ = handle_client(stream, state, rx).await;
                });
            }
            _ = shutdown.recv() => {
                tracing::info!("session shutting down");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_client(
    stream: UnixStream,
    state: SessionState,
    mut rx: broadcast::Receiver<SessionEvent>,
) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();

    loop {
        tokio::select! {
            n = reader.read_line(&mut line) => {
                if n? == 0 { break; }
                let cmd = line.trim();
                if !cmd.is_empty() {
                    if let Ok(resp) = handle_command(cmd, &state).await {
                        write.write_all(resp.as_bytes()).await?;
                        write.write_all(b"\n").await?;
                    }
                }
                line.clear();
            }
            evt = rx.recv() => {
                if let Ok(evt) = evt {
                    if let Ok(json) = serde_json::to_string(&evt) {
                        let _ = write.write_all(json.as_bytes()).await;
                        let _ = write.write_all(b"\n").await;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_command(cmd: &str, state: &SessionState) -> Result<String> {
    match cmd {
        "get_battery" => {
            let battery = state.battery.read().await;
            Ok(serde_json::to_string(&*battery)?)
        }
        "get_media" => {
            let media = state.media.read().await;
            Ok(serde_json::to_string(&*media)?)
        }
        "get_power" => {
            let power = state.power.read().await;
            Ok(serde_json::to_string(&*power)?)
        }
        "get_all" => {
            let battery = state.battery.read().await.clone();
            let media = state.media.read().await.clone();
            let power = state.power.read().await.clone();
            Ok(serde_json::to_string(&json!({
                "battery": battery,
                "media": media,
                "power": power,
            }))?)
        }
        _ => Ok(json!({"error": "unknown command"}).to_string()),
    }
}
