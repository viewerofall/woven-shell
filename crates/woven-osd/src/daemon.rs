//! Daemon loop — Wayland event loop + Unix socket listener via background thread.

use anyhow::Result;
use crossbeam_channel::{bounded, TryRecvError};
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;

use crate::read::{read_brightness, read_media, read_volume};
use crate::render::render;
use crate::state::{OsdKind, OsdState, Phase};
use crate::surface::OsdSurface;
use crate::SOCK;

pub fn run() -> Result<()> {
    // Clean up stale socket
    let _ = std::fs::remove_file(SOCK);

    let (tx, rx) = bounded::<String>(32);

    // Spawn socket listener thread
    let listener = UnixListener::bind(SOCK)?;
    std::thread::spawn(move || socket_thread(listener, tx));

    let font = load_font();
    let mut surf  = OsdSurface::new()?;
    let mut state = OsdState::new();

    // Wait for Wayland configure
    for _ in 0..100 {
        surf.dispatch()?;
        if surf.configured() { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Initial blank frame so surface is committed
    let blank = render(&state, &font);
    let _ = surf.present(&blank);

    loop {
        surf.dispatch()?;

        // Drain IPC commands
        loop {
            match rx.try_recv() {
                Ok(cmd) => handle_cmd(cmd.trim(), &mut state),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    tracing::warn!("osd: socket thread died");
                    break;
                }
            }
        }

        // Advance animation
        let needs_repaint = state.tick();

        if needs_repaint || state.visible() {
            let pixels = render(&state, &font);
            let _ = surf.present(&pixels);
        }

        // Sleep a bit when hidden to avoid burning CPU
        if state.phase == Phase::Hidden {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}

fn handle_cmd(cmd: &str, state: &mut OsdState) {
    match cmd {
        "volume" => {
            let v = read_volume();
            state.show(OsdKind::Volume(v));
        }
        "bright" => {
            let b = read_brightness();
            state.show(OsdKind::Brightness(b));
        }
        "media" => {
            // Small delay so playerctl state has updated
            std::thread::sleep(std::time::Duration::from_millis(80));
            if let Some(m) = read_media() {
                state.show(OsdKind::Media(m));
            }
        }
        "ping" => {} // keepalive check from client
        other => tracing::debug!("osd: unknown cmd '{other}'"),
    }
}

fn socket_thread(listener: UnixListener, tx: crossbeam_channel::Sender<String>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let tx = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stream);
            for line in reader.lines() {
                match line {
                    Ok(l) if !l.is_empty() => { let _ = tx.send(l); }
                    _ => break,
                }
            }
        });
    }
}

fn load_font() -> fontdue::Font {
    // Try Nerd Font first for icons, fall back to regular
    for p in &[
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/FiraCodeNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/HackNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/Inconsolata-Regular.ttf",
        "/usr/share/fonts/TTF/Inconsolata.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(f) = fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()) {
                return f;
            }
        }
    }
    panic!("woven-osd: no font found");
}
