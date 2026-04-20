//! System state readers — volume, brightness, media.

#[derive(Debug, Clone)]
pub struct VolumeState {
    pub level:  u8,
    pub muted:  bool,
    pub device: String,
}

#[derive(Debug, Clone)]
pub struct MediaState {
    pub title:   String,
    pub artist:  String,
    pub playing: bool,
}

// ── Volume ────────────────────────────────────────────────────────────────────

pub fn read_volume() -> VolumeState {
    let level;
    let muted;

    if let Ok(out) = std::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        muted  = s.contains("[MUTED]");
        level  = s.split_whitespace()
            .find(|w| w.parse::<f32>().is_ok())
            .and_then(|w| w.parse::<f32>().ok())
            .map(|v| (v * 100.0).min(100.0) as u8)
            .unwrap_or(0);
    } else {
        level = 0;
        muted = false;
    }

    let device = read_sink_name();
    VolumeState { level, muted, device }
}

fn read_sink_name() -> String {
    // wpctl inspect gives "node.description = "Device Name""
    if let Ok(out) = std::process::Command::new("wpctl")
        .args(["inspect", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("node.description") {
                if let Some(val) = line.splitn(2, '=').nth(1) {
                    let name = val.trim().trim_matches('"').trim().to_string();
                    if !name.is_empty() { return name; }
                }
            }
        }
    }
    "Audio".to_string()
}

// ── Brightness ────────────────────────────────────────────────────────────────

pub fn read_brightness() -> u8 {
    if let Ok(out) = std::process::Command::new("brightnessctl")
        .args(["-m", "g"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(pct) = s.trim().split(',').nth(5) {
            if let Ok(v) = pct.trim_end_matches('%').parse::<u8>() {
                return v;
            }
        }
    }
    // sysfs fallback
    if let Ok(entries) = std::fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let base = entry.path();
            let cur: u64 = std::fs::read_to_string(base.join("brightness"))
                .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let max: u64 = std::fs::read_to_string(base.join("max_brightness"))
                .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1);
            if max > 0 { return ((cur * 100 / max) as u8).min(100); }
        }
    }
    0
}

// ── Media ─────────────────────────────────────────────────────────────────────

pub fn read_media() -> Option<MediaState> {
    let status = run("playerctl", &["status"]).ok()?;
    let status = status.trim();
    if status == "No players found" || status.is_empty() { return None; }
    let playing = status == "Playing";

    let title  = run("playerctl", &["metadata", "title"]).unwrap_or_default();
    let artist = run("playerctl", &["metadata", "artist"]).unwrap_or_default();
    let title  = title.trim().to_string();
    let artist = artist.trim().to_string();

    if title.is_empty() && artist.is_empty() { return None; }

    Some(MediaState { title, artist, playing })
}

fn run(cmd: &str, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new(cmd).args(args).output()?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
