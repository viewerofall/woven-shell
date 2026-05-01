mod battery;
mod command;
mod media;
mod socket;
mod state;

use anyhow::Result;
use std::time::Duration;
use tokio::sync::mpsc;

use battery::BatteryReader;
use command::run_command;
use media::MediaReader;
use socket::run_socket_server;
use state::{BatteryState, MediaState, SessionState};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // No args: print help
    if args.len() < 2 {
        return run_command(&["help".to_string()]).await;
    }

    // Run command mode
    run_command(&args[1..]).await
}

pub async fn run_daemon() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("woven-session starting...");

    let state = SessionState::new();
    let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

    // Spawn socket server
    let state_clone = SessionState {
        battery: state.battery.clone(),
        media: state.media.clone(),
        power: state.power.clone(),
    };
    tokio::spawn(async move {
        if let Err(e) = run_socket_server(state_clone, shutdown_rx).await {
            tracing::error!("socket server error: {}", e);
        }
    });

    // Setup signal handler for graceful shutdown
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("received SIGINT");
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Main update loop
    loop {
        // Update battery every 5s
        if let Ok((percent, ac_online)) = BatteryReader::read() {
            let new_state = BatteryState { percent, ac_online };
            let mut battery = state.battery.write().await;
            if battery.percent != new_state.percent || battery.ac_online != new_state.ac_online {
                *battery = new_state;
                tracing::debug!("battery updated: {}% ac={}", percent, ac_online);
            }
        }

        // Update media every 2s
        if let Ok((playing, title, artist)) = MediaReader::read().await {
            let new_state = MediaState {
                playing,
                title: title.clone(),
                artist: artist.clone(),
            };
            let mut media = state.media.write().await;
            if media.playing != new_state.playing
                || media.title != new_state.title
                || media.artist != new_state.artist
            {
                tracing::debug!("media updated: playing={} title={}", playing, title);
                *media = new_state;
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
