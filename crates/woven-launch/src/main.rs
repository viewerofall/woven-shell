//! woven-launch — elegant app launcher for woven-shell.
//!
//! Usage:  woven-launch          — open the launcher overlay
//!
//! Features: fuzzy .desktop search, = calculator, ! command runner.
//! Keyboard: type to search, ↑↓ navigate, Enter launch, Esc close.

mod calc;
mod config;
mod desktop;
mod draw;
mod icons;
mod render;
mod search;
mod text;
mod wayland;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("woven_launch=info".parse().unwrap()),
        )
        .init();

    let cfg = config::LaunchConfig::load();
    let entries = desktop::collect_entries();
    tracing::info!("launch: loaded {} desktop entries", entries.len());

    let mut renderer = render::LaunchRenderer::new(cfg.launcher, entries);
    let mut surface = wayland::LaunchSurface::new()?;

    surface.ensure_surface();
    let _ = surface.dispatch();

    // wait for configure
    for _ in 0..150 {
        surface.dispatch()?;
        if surface.configured() { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !surface.configured() {
        anyhow::bail!("launch: compositor never configured the surface");
    }

    tracing::info!("launch: ready {:?}", surface.size());

    loop {
        surface.dispatch()?;

        // process input
        for input in surface.drain_input() {
            match input {
                wayland::LaunchInput::Char(ch) => renderer.push_char(ch),
                wayland::LaunchInput::Backspace => renderer.pop_char(),
                wayland::LaunchInput::Escape => {
                    if renderer.query.is_empty() {
                        renderer.should_close = true;
                    } else {
                        renderer.clear_query();
                    }
                }
                wayland::LaunchInput::Enter => renderer.confirm(),
                wayland::LaunchInput::Up => renderer.select_up(),
                wayland::LaunchInput::Down | wayland::LaunchInput::Tab => renderer.select_down(),
                wayland::LaunchInput::PageUp => renderer.page_up(),
                wayland::LaunchInput::PageDown => renderer.page_down(),
                wayland::LaunchInput::Scroll(dy) => renderer.scroll(dy),
                wayland::LaunchInput::Click(mx, my) => renderer.handle_click(mx, my),
                wayland::LaunchInput::MouseMove(mx, my) => renderer.handle_mouse_move(mx, my),
            }
        }

        if renderer.should_close { break; }

        // render
        let (w, h) = surface.size();
        if w > 0 && h > 0 && renderer.dirty {
            let pixels = renderer.render(w, h);
            surface.present(pixels)?;
        }

        // cursor blink needs re-render
        if renderer.is_animating() {
            renderer.dirty = true;
        }

        let delay = if renderer.is_animating() { 16 } else { 33 };
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }

    // launch the selected app/command
    if let Some(exec) = renderer.launch_exec.take() {
        tracing::info!("launch: exec {exec}");
        spawn_detached(&exec);
    }

    Ok(())
}

/// Spawn a process detached from the launcher (setsid so it survives).
fn spawn_detached(cmd: &str) {
    use std::process::Command;
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return; }

    match Command::new("setsid")
        .arg("-f")
        .args(&parts)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => tracing::info!("launch: spawned: {cmd}"),
        Err(e) => tracing::error!("launch: spawn failed: {e}"),
    }
}
