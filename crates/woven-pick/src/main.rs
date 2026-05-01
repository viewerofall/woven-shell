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
    let home = std::env::var("HOME").unwrap_or_default();

    let live_path = format!("{home}/.config/woven-shell/wall.toml");
    let existing_config = std::fs::read_to_string(&live_path).unwrap_or_default();

    let is_slideshow = existing_config.lines().any(|l| {
        let l = l.trim();
        l.starts_with("type") && l.contains("\"slideshow\"")
    });

    // Get the selected image's parent directory (normalized)
    let selected_dir = if let Some(parent) = path.parent() {
        let s = parent.display().to_string();
        if s.starts_with(&home) { format!("~{}", &s[home.len()..]) } else { s }
    } else {
        "~/Pictures/Wallpapers".to_string()
    };

    if is_slideshow {
        // Extract current dir from config — reuse same parsing as read_dir_from_config
        let current_dir = existing_config.lines()
            .find(|l| l.trim().starts_with("dir"))
            .and_then(|l| l.splitn(2, '=').nth(1))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_default();

        // Normalize both to absolute paths for comparison
        let current_abs = if current_dir.starts_with('~') {
            format!("{home}{}", &current_dir[1..])
        } else {
            current_dir.clone()
        };
        let selected_abs = if selected_dir.starts_with('~') {
            format!("{home}{}", &selected_dir[1..])
        } else {
            selected_dir.clone()
        };
        let same_dir = current_abs.trim_end_matches('/') == selected_abs.trim_end_matches('/');

        if same_dir {
            // Same directory — just IPC set to jump to this image, slideshow continues
            tracing::info!("pick: same dir ({current_dir}), skipping config rewrite");
        } else {
            // Directory changed — rewrite config preserving other settings
            let shuffle = if existing_config.lines().any(|l| {
                let l = l.trim(); l.starts_with("shuffle") && l.contains("true")
            }) { "true" } else { "false" };
            let interval = existing_config.lines()
                .find(|l| l.trim().starts_with("interval"))
                .and_then(|l| l.splitn(2, '=').nth(1))
                .map(|v| v.trim().to_string())
                .unwrap_or_else(|| "300".to_string());
            let transition = existing_config.lines()
                .find(|l| l.trim().starts_with("transition") && !l.contains("transition_secs"))
                .and_then(|l| l.splitn(2, '=').nth(1))
                .map(|v| v.trim().trim_matches('"').to_string())
                .unwrap_or_else(|| "pixelate".to_string());
            let transition_secs = existing_config.lines()
                .find(|l| l.trim().starts_with("transition_secs"))
                .and_then(|l| l.splitn(2, '=').nth(1))
                .map(|v| v.trim().to_string())
                .unwrap_or_else(|| "1.5".to_string());

            let content = format!(
                "# woven-wall config — ~/.config/woven-shell/wall.toml\n\
                 \n\
                 [wallpaper]\n\
                 type            = \"slideshow\"\n\
                 dir             = \"{selected_dir}\"\n\
                 interval        = {interval}\n\
                 transition      = \"{transition}\"\n\
                 transition_secs = {transition_secs}\n\
                 shuffle         = {shuffle}\n"
            );
            let repo_path = format!("{home}/woven-shell/config/wall.toml");
            std::fs::write(&repo_path, &content)?;
            std::fs::write(&live_path, &content)?;
            tracing::info!("pick: dir changed → rewrote config");
        }
    } else {
        // Was static image — switch to slideshow mode
        let content = format!(
            "# woven-wall config — ~/.config/woven-shell/wall.toml\n\
             \n\
             [wallpaper]\n\
             type            = \"slideshow\"\n\
             dir             = \"{selected_dir}\"\n\
             interval        = 300\n\
             transition      = \"pixelate\"\n\
             transition_secs = 1.5\n\
             shuffle         = false\n"
        );
        let repo_path = format!("{home}/woven-shell/config/wall.toml");
        std::fs::write(&repo_path, &content)?;
        std::fs::write(&live_path, &content)?;
        tracing::info!("pick: was static, switched to slideshow");
    }

    // Always IPC the specific image to show it immediately
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
