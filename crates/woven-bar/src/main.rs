//! woven-bar — standalone Wayland layer-shell status bar
//! Part of the woven-shell suite.

mod wayland;
mod draw;
mod text;
mod icons;
mod sway;
mod config;
mod bar;
mod widgets;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("woven_bar=info".parse().unwrap()))
        .init();

    let cfg = config::BarConfig::load()?;
    bar::run(cfg)
}
