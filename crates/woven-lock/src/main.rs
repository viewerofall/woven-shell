//! woven-lock — elegant Wayland lock screen for woven-shell.
//!
//! Usage:  woven-lock              — lock the session
//!
//! Background is configured independently in lock.toml (not tied to woven-wall).
//! Supports a fixed image or random pick from a directory on each lock.
//! Authenticates via PAM.

mod auth;
mod blur;
mod config;
mod draw;
mod render;
mod text;
mod wayland;

use anyhow::{bail, Result};
use render::{LockPhase, LockRenderer};
use std::collections::HashMap;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("woven_lock=info".parse().unwrap()),
        )
        .init();

    let cfg = config::LockConfig::load();
    let blur_radius = cfg.lock.blur_radius;
    let bg_settings = cfg.background.clone();
    let mut renderer = LockRenderer::new(cfg.lock);
    let mut surface = wayland::LockSurface::new()?;

    // wait for outputs
    for _ in 0..100 {
        surface.dispatch()?;
        if surface.configured_count() > 0 { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if surface.configured_count() == 0 {
        bail!("lock: no outputs became available");
    }

    tracing::info!("lock: {} output(s) locked", surface.configured_count());

    // load and blur wallpaper for each output size
    let mut bg_cache: HashMap<(u32, u32), Vec<u8>> = HashMap::new();
    for (w, h) in surface.output_sizes() {
        if !bg_cache.contains_key(&(w, h)) {
            let bg = match blur::load_blurred_wallpaper(&bg_settings, w, h, blur_radius) {
                Ok(b) => {
                    tracing::info!("lock: blurred wallpaper {}×{}", w, h);
                    b
                }
                Err(e) => {
                    tracing::warn!("lock: wallpaper load failed: {e}, using solid bg");
                    blur::solid_background(w, h)
                }
            };
            bg_cache.insert((w, h), bg);
        }
    }

    // set the background on the renderer (use first output's size for the renderer)
    if let Some((&(w, h), bg)) = bg_cache.iter().next() {
        renderer.set_background(w, h, bg.clone());
    }

    // main loop
    let mut auth_thread: Option<std::thread::JoinHandle<auth::AuthResult>> = None;

    loop {
        surface.dispatch()?;

        // process input events
        for input in surface.drain_input() {
            match input {
                wayland::LockInput::Char(ch) => {
                    if !matches!(renderer.phase, LockPhase::Verifying | LockPhase::Unlocking) {
                        renderer.push_char(ch);
                    }
                }
                wayland::LockInput::Backspace => {
                    if !matches!(renderer.phase, LockPhase::Verifying | LockPhase::Unlocking) {
                        renderer.pop_char();
                    }
                }
                wayland::LockInput::Escape => {
                    if !matches!(renderer.phase, LockPhase::Verifying | LockPhase::Unlocking) {
                        renderer.clear_password();
                    }
                }
                wayland::LockInput::Enter => {
                    if !renderer.password.is_empty()
                        && !matches!(renderer.phase, LockPhase::Verifying | LockPhase::Unlocking)
                    {
                        renderer.start_verify();
                        let pw = renderer.password.clone();
                        auth_thread = Some(std::thread::spawn(move || auth::authenticate(&pw)));
                    }
                }
            }
        }

        // check auth result
        if let Some(ref handle) = auth_thread {
            if handle.is_finished() {
                let handle = auth_thread.take().unwrap();
                match handle.join() {
                    Ok(auth::AuthResult::Success) => {
                        tracing::info!("lock: authenticated");
                        renderer.start_unlock();
                    }
                    Ok(auth::AuthResult::Failed) => {
                        tracing::info!("lock: wrong password");
                        renderer.show_error();
                    }
                    Ok(auth::AuthResult::Error(e)) => {
                        tracing::error!("lock: PAM error: {e}");
                        renderer.show_error();
                    }
                    Err(_) => {
                        tracing::error!("lock: auth thread panicked");
                        renderer.show_error();
                    }
                }
            }
        }

        // transition error → idle after shake animation
        renderer.maybe_reset_error();

        // exit after unlock fade
        if renderer.unlock_done() {
            tracing::info!("lock: unlocked, exiting");
            break;
        }

        // render all outputs
        surface.present_all(|w, h| {
            // update bg if this output size differs from what's cached in renderer
            if let Some(bg) = bg_cache.get(&(w, h)) {
                renderer.set_background(w, h, bg.clone());
            }
            renderer.render(w, h)
        })?;

        // adaptive sleep: faster during animations, slower when idle
        let delay = if renderer.is_animating() { 16 } else { 50 };
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }

    Ok(())
}
