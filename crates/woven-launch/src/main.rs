//! woven-launch: minimal Wayland client

mod calc;
mod config;
mod desktop;
mod draw;
mod render;
mod search;
mod text;

use anyhow::Result;
use config::LaunchConfig;
use render::LaunchRenderer;
use std::sync::{Arc, Mutex};
use wayland_client::{Connection, protocol::wl_registry};

struct AppState {
    renderer: Arc<Mutex<LaunchRenderer>>,
    should_exit: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .init();

    tracing::info!("woven-launch starting");

    let cfg = LaunchConfig::load();
    let entries = desktop::collect_entries();
    tracing::info!("Loaded {} desktop entries", entries.len());

    let renderer = Arc::new(Mutex::new(LaunchRenderer::new(cfg.launcher, entries)));

    let conn = Connection::connect_to_env()
    .map_err(|e| anyhow::anyhow!("Wayland connection failed: {}", e))?;

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let app = Arc::new(Mutex::new(AppState {
        renderer,
        should_exit: false,
    }));

    tracing::info!("Connected to Wayland, entering event loop");

    loop {
        event_queue.blocking_dispatch(&mut ())?;

        let should_exit = app.lock().unwrap().should_exit;
        if should_exit {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    tracing::info!("woven-launch exiting");
    Ok(())
}
