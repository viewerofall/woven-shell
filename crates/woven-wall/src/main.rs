//! woven-wall — Wayland background wallpaper daemon
//! Part of the woven-shell suite.
//!
//! Usage:
//!   woven-wall                        — load from ~/.config/woven-shell/wall.toml
//!   woven-wall -i <path>              — static image (PNG/JPG)
//!   woven-wall -g <path>              — animated GIF
//!   woven-wall -v <path>              — video (mp4, mkv, …) via ffmpeg
//!   woven-wall -c <#rrggbb>           — solid color
//!   woven-wall --gradient <colors>    — animated gradient, colors comma-separated
//!   woven-wall --slideshow [dir]      — slideshow from directory (default transition: pixelate)
//!
//! IPC (while daemon is running):
//!   woven-wall next                   — skip to next wallpaper
//!   woven-wall prev                   — go to previous wallpaper
//!   woven-wall set <path>             — switch to a specific image

mod wayland;
mod config;
mod sources;
mod theme;

use anyhow::{bail, Result};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Instant, SystemTime};
use tracing_subscriber::EnvFilter;
use config::WallpaperKind;

fn socket_path() -> String {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    format!("{runtime}/woven-wall.sock")
}

fn ipc_send(cmd: &str) -> Result<()> {
    let mut stream = UnixStream::connect(socket_path())?;
    stream.write_all(cmd.as_bytes())?;
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("woven_wall=info".parse().unwrap()))
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();

    // IPC client mode — forward command to running daemon
    if matches!(args.first().map(|s| s.as_str()), Some("next" | "prev" | "set")) {
        let cmd = args.join(" ");
        return ipc_send(&cmd).map_err(|e| anyhow::anyhow!("IPC failed (daemon running?): {e}"));
    }

    let from_config = args.is_empty();
    let kind = parse_args()?;
    let mut src = sources::build(&kind)?;
    let mut surface = wayland::WallSurface::new()?;

    for _ in 0..50 {
        surface.dispatch()?;
        if surface.output_count() > 0 { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if surface.output_count() == 0 {
        bail!("no Wayland outputs became available");
    }

    // bind IPC socket
    let sock_path = socket_path();
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;
    listener.set_nonblocking(true)?;
    tracing::info!("wall: IPC socket at {sock_path}");

    let mut last_cfg_mtime = if from_config { config_mtime() } else { None };
    let mut last_cfg_check = Instant::now();

    // extract theme from the initial wallpaper
    let first_size = surface.first_output_size();
    if let Some((fw, fh)) = first_size {
        let frame = src.frame(fw, fh);
        theme::extract_and_write(&frame, fw, fh);
        let _ = src.wallpaper_changed(); // consume the flag
    }

    loop {
        surface.dispatch()?;
        let mut last_frame: Option<(Vec<u8>, u32, u32)> = None;
        surface.present_for_each(|w, h| {
            let f = src.frame(w, h);
            last_frame = Some((f.clone(), w, h));
            f
        })?;

        // extract theme when wallpaper changes
        if src.wallpaper_changed() {
            if let Some((ref frame, w, h)) = last_frame {
                theme::extract_and_write(frame, w, h);
            }
        }

        // drain IPC commands
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(50)));
                    let mut buf = [0u8; 512];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let buf = String::from_utf8_lossy(&buf[..n]).to_string();
                    let cmd = buf.trim();
                    if !cmd.is_empty() {
                        tracing::info!("wall: IPC ← {cmd}");
                        src.handle_ipc(cmd);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => { tracing::debug!("wall: IPC accept error: {e}"); break; }
            }
        }

        if from_config && last_cfg_check.elapsed().as_secs() >= 2 {
            last_cfg_check = Instant::now();
            let mtime = config_mtime();
            if mtime != last_cfg_mtime {
                last_cfg_mtime = mtime;
                match config::WallConfig::load().and_then(|c| sources::build(&c.wallpaper)) {
                    Ok(new_src) => {
                        src = new_src;
                        tracing::info!("wall: config reloaded");
                    }
                    Err(e) => tracing::warn!("wall: config reload failed: {e}"),
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(src.frame_delay_ms()));
    }
}

fn config_mtime() -> Option<SystemTime> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    std::fs::metadata(format!("{home}/.config/woven-shell/wall.toml"))
        .and_then(|m| m.modified())
        .ok()
}

fn parse_args() -> Result<WallpaperKind> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        return Ok(config::WallConfig::load()?.wallpaper);
    }

    let flag = args[0].as_str();
    let val  = args.get(1).map(|s| s.as_str());

    match flag {
        "-i" | "--image" => {
            let path = val.ok_or_else(|| anyhow::anyhow!("-i requires a path"))?.to_string();
            Ok(WallpaperKind::Image { path })
        }
        "-g" | "--gif" => {
            let path = val.ok_or_else(|| anyhow::anyhow!("-g requires a path"))?.to_string();
            Ok(WallpaperKind::Gif { path })
        }
        "-v" | "--video" => {
            let path = val.ok_or_else(|| anyhow::anyhow!("-v requires a path"))?.to_string();
            Ok(WallpaperKind::Video { path })
        }
        "-c" | "--color" => {
            let color = val.ok_or_else(|| anyhow::anyhow!("-c requires a hex color"))?.to_string();
            Ok(WallpaperKind::Color { color })
        }
        "--gradient" => {
            let colors = val
                .ok_or_else(|| anyhow::anyhow!("--gradient requires comma-separated hex colors"))?
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            Ok(WallpaperKind::Gradient { colors, duration: 30.0 })
        }
        "--slideshow" => {
            let dir = val.unwrap_or("~/Pictures/Wallpapers").to_string();
            Ok(WallpaperKind::Slideshow {
                dir,
                interval:        300,
                transition:      config::TransitionKind::Pixelate,
                transition_secs: 1.5,
                shuffle:         false,
            })
        }
        "-h" | "--help" => {
            eprintln!("woven-wall [FLAG <value>]");
            eprintln!("  (no args)               load from ~/.config/woven-shell/wall.toml");
            eprintln!("  -i <path>               static image (PNG/JPG)");
            eprintln!("  -g <path>               animated GIF");
            eprintln!("  -v <path>               video via ffmpeg");
            eprintln!("  -c <#rrggbb>            solid color");
            eprintln!("  --gradient <c1,c2,…>    animated gradient");
            eprintln!("  --slideshow [dir]        slideshow from dir (default: ~/Pictures/Wallpapers)");
            std::process::exit(0);
        }
        other => bail!("unknown flag: {other}  (try --help)"),
    }
}
