//! woven-pick — wallpaper picker overlay for woven-shell.
//!
//! Usage:  woven-pick           (reads dir from ~/.config/woven-shell/wall.toml)
//!         woven-pick <dir>     (explicit directory)
//!
//! On selection: updates ~/woven-shell/config/wall.toml AND ~/.config/woven-shell/wall.toml,
//!               then sends IPC to woven-wall for immediate apply.

mod draw;
mod picker;
mod text;
mod wayland;

use anyhow::Result;
use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("woven_pick=info".parse().unwrap()))
        .init();

    let dir = match std::env::args().nth(1) {
        Some(d) => d,
        None    => read_dir_from_config(),
    };
    tracing::info!("pick: wallpaper dir = {dir}");

    let picker  = picker::Picker::new(&dir)?;
    let mut surface = wayland::PickSurface::new(picker)?;

    // create the layer surface now that globals are bound
    surface.ensure_surface();
    let _ = surface.dispatch(); // flush the commit

    // wait for compositor configure (Sway sends it after the commit roundtrip)
    for _ in 0..150 {
        surface.dispatch()?;
        if surface.configured() { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    if !surface.configured() {
        anyhow::bail!("compositor never configured the surface — is wlr-layer-shell supported?");
    }

    tracing::info!("pick: ready {:?}", surface.size());

    loop {
        surface.dispatch()?;
        if surface.should_close() { break; }

        let (w, h) = surface.size();
        if w > 0 && h > 0 && surface.needs_render() {
            let pixels = surface.render_frame(w, h);
            surface.present(pixels)?;
        }

        // 16ms while animating, 33ms otherwise — saves CPU when idle
        let delay = if surface.needs_render() { 16 } else { 33 };
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }

    if let Some(path) = surface.apply_path() {
        apply_wallpaper(&path)?;
    }

    Ok(())
}

fn apply_wallpaper(path: &PathBuf) -> Result<()> {
    let display_path = path.display().to_string();

    // normalise to ~/... form if possible
    let home = std::env::var("HOME").unwrap_or_default();
    let toml_path = if display_path.starts_with(&home) {
        format!("~{}", &display_path[home.len()..])
    } else {
        display_path.clone()
    };

    let toml_content = format!(
        "# woven-wall config — ~/.config/woven-shell/wall.toml\n\
         \n\
         [wallpaper]\n\
         type = \"image\"\n\
         path = \"{toml_path}\"\n\
         \n\
         # ── Slideshow ────────────────────────────────────────────────────────────────\n\
         # [wallpaper]\n\
         # type            = \"slideshow\"\n\
         # dir             = \"~/Pictures/Wallpapers\"\n\
         # interval        = 300\n\
         # transition      = \"pixelate\"\n\
         # transition_secs = 1.5\n\
         # shuffle         = false\n"
    );

    // write repo config
    let repo_path = format!("{home}/woven-shell/config/wall.toml");
    std::fs::write(&repo_path, &toml_content)?;
    tracing::info!("pick: wrote {repo_path}");

    // copy to live config
    let live_path = format!("{home}/.config/woven-shell/wall.toml");
    std::fs::write(&live_path, &toml_content)?;
    tracing::info!("pick: wrote {live_path}");

    // send IPC to running woven-wall daemon
    let sock = format!("{}/woven-wall.sock",
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()));
    match UnixStream::connect(&sock) {
        Ok(mut stream) => {
            let _ = stream.write_all(format!("set {display_path}").as_bytes());
            tracing::info!("pick: IPC set {display_path}");
        }
        Err(e) => tracing::warn!("pick: IPC failed (daemon not running?): {e}"),
    }

    println!("Applied: {display_path}");
    Ok(())
}

fn read_dir_from_config() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{home}/.config/woven-shell/wall.toml");
    if let Ok(s) = std::fs::read_to_string(&path) {
        // quick manual parse — just find dir = "..."
        for line in s.lines() {
            let line = line.trim();
            if line.starts_with("dir") {
                if let Some(v) = line.splitn(2, '=').nth(1) {
                    let v = v.trim().trim_matches('"').trim();
                    if !v.is_empty() { return v.to_string(); }
                }
            }
        }
    }
    "~/Pictures/Wallpapers".to_string()
}
