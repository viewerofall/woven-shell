//! Config reading, writing, and sway keybind file generation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Paths ─────────────────────────────────────────────────────────────────────

fn cfg_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/woven-shell")
}

fn sway_keybinds_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/sway/woven-keybinds")
}

// ── Bar config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarCfg {
    #[serde(default = "default_bar_height")]
    pub height: u32,
    #[serde(default = "default_position")]
    pub position: String,
    #[serde(default)]
    pub theme_source: String,
    #[serde(default)]
    pub theme: BarTheme,
    #[serde(default)]
    pub bubbles: BubblesCfg,
    #[serde(default)]
    pub modules: ModulesCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BarTheme {
    #[serde(default = "def_bg")]      pub background: String,
    #[serde(default = "def_fg")]      pub foreground: String,
    #[serde(default = "def_accent")]  pub accent: String,
    #[serde(default = "def_dim")]     pub dim: String,
    #[serde(default = "def_radius")]  pub radius: u32,
    #[serde(default = "def_font")]    pub font_size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BubblesCfg {
    #[serde(default = "def_bubble_bg")] pub background: String,
    #[serde(default = "def_radius")]    pub radius: u32,
    #[serde(default = "def_gap")]       pub gap: u32,
    #[serde(default = "def_padding")]   pub padding: u32,
    #[serde(default = "def_margin")]    pub margin: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModulesCfg {
    #[serde(default)] pub left:   Vec<String>,
    #[serde(default)] pub center: Vec<String>,
    #[serde(default)] pub right:  Vec<String>,
}

fn default_bar_height() -> u32 { 34 }
fn default_position()   -> String { "top".into() }
fn def_bg()       -> String { "#0a0010".into() }
fn def_fg()       -> String { "#cdd6f4".into() }
fn def_accent()   -> String { "#c792ea".into() }
fn def_dim()      -> String { "#2a1545".into() }
fn def_radius()   -> u32    { 7 }
fn def_font()     -> f32    { 13.0 }
fn def_bubble_bg()-> String { "#1a0a2e".into() }
fn def_gap()      -> u32    { 6 }
fn def_padding()  -> u32    { 10 }
fn def_margin()   -> u32    { 4 }

impl Default for BarCfg {
    fn default() -> Self {
        Self {
            height: 34,
            position: "top".into(),
            theme_source: "config".into(),
            theme: BarTheme::default(),
            bubbles: BubblesCfg::default(),
            modules: ModulesCfg {
                left:   vec!["activities".into(), "workspaces".into(), "window_title".into()],
                center: vec!["clock".into()],
                right:  vec!["systray".into(), "notifications".into(), "media".into(),
                             "cpu".into(), "memory".into(), "audio".into(), "battery".into(),
                             "control_center".into()],
            },
        }
    }
}

// ── Wall config ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallCfg {
    #[serde(default)] pub wallpaper: WallpaperCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WallpaperCfg {
    #[serde(rename = "type", default = "def_wall_type")]
    pub kind: String,
    #[serde(default)] pub path:            String,
    #[serde(default)] pub color:           String,
    #[serde(default)] pub dir:             String,
    #[serde(default)] pub colors:          Vec<String>,
    #[serde(default = "def_interval")]
    pub interval:       u32,
    #[serde(default = "def_transition")]
    pub transition:     String,
    #[serde(default = "def_trans_secs")]
    pub transition_secs: f32,
    #[serde(default)] pub shuffle:         bool,
    #[serde(default = "def_duration")]
    pub duration:       f32,
}

fn def_wall_type()   -> String { "image".into() }
fn def_interval()    -> u32   { 300 }
fn def_transition()  -> String { "pixelate".into() }
fn def_trans_secs()  -> f32   { 1.5 }
fn def_duration()    -> f32   { 30.0 }

impl Default for WallCfg {
    fn default() -> Self { Self { wallpaper: WallpaperCfg::default() } }
}

// ── Lock config ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockCfg {
    #[serde(default)] pub background: LockBgCfg,
    #[serde(default)] pub lock:       LockSettingsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockBgCfg {
    #[serde(rename = "type", default = "def_lock_bg_type")]
    pub kind: String,
    #[serde(default)] pub dir:  String,
    #[serde(default)] pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockSettingsCfg {
    #[serde(default = "def_blur")]      pub blur_radius:    u32,
    #[serde(default = "def_true")]      pub show_clock:     bool,
    #[serde(default = "def_clock_fmt")] pub clock_format:   String,
    #[serde(default = "def_true")]      pub show_date:      bool,
    #[serde(default = "def_date_fmt")]  pub date_format:    String,
    #[serde(default = "def_lock_fg")]   pub text_color:     String,
    #[serde(default = "def_lock_ac")]   pub accent_color:   String,
    #[serde(default = "def_lock_err")]  pub error_color:    String,
    #[serde(default = "def_fade")]      pub fade_in_ms:     u32,
    #[serde(default = "def_fade")]      pub fade_out_ms:    u32,
    #[serde(default = "def_true")]      pub shake_on_error: bool,
}

fn def_lock_bg_type() -> String { "random".into() }
fn def_blur()         -> u32    { 20 }
fn def_true()         -> bool   { true }
fn def_clock_fmt()    -> String { "%H:%M".into() }
fn def_date_fmt()     -> String { "%A, %B %e".into() }
fn def_lock_fg()      -> String { "#cdd6f4".into() }
fn def_lock_ac()      -> String { "#cba6f7".into() }
fn def_lock_err()     -> String { "#f07178".into() }
fn def_fade()         -> u32    { 200 }

impl Default for LockBgCfg {
    fn default() -> Self {
        Self { kind: "random".into(), dir: "~/Pictures/Wallpapers".into(), path: String::new() }
    }
}
impl Default for LockSettingsCfg {
    fn default() -> Self {
        Self {
            blur_radius: 20, show_clock: true, clock_format: "%H:%M".into(),
            show_date: true,  date_format: "%A, %B %e".into(),
            text_color: "#cdd6f4".into(), accent_color: "#cba6f7".into(),
            error_color: "#f07178".into(), fade_in_ms: 200, fade_out_ms: 200,
            shake_on_error: true,
        }
    }
}
impl Default for LockCfg {
    fn default() -> Self { Self { background: Default::default(), lock: Default::default() } }
}

// ── Launch config ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchCfg {
    #[serde(default)] pub launcher: LauncherCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherCfg {
    #[serde(default = "def_launch_w")]      pub width:            u32,
    #[serde(default = "def_max_results")]   pub max_results:      u32,
    #[serde(default = "def_bg")]            pub background:       String,
    #[serde(default = "def_panel_bg")]      pub panel_background: String,
    #[serde(default = "def_launch_fg")]     pub text_color:       String,
    #[serde(default = "def_launch_dim")]    pub text_dim:         String,
    #[serde(default = "def_accent")]        pub accent_color:     String,
    #[serde(default = "def_selection")]     pub selection_color:  String,
    #[serde(default = "def_accent")]        pub border_color:     String,
    #[serde(default = "def_true")]          pub calculator:       bool,
    #[serde(default = "def_true")]          pub command_runner:   bool,
}

fn def_launch_w()    -> u32   { 620 }
fn def_max_results() -> u32   { 8 }
fn def_panel_bg()    -> String { "#0f0020".into() }
fn def_launch_fg()   -> String { "#e8e0f0".into() }
fn def_launch_dim()  -> String { "#8888aa".into() }
fn def_selection()   -> String { "#1a0035".into() }

impl Default for LauncherCfg {
    fn default() -> Self {
        Self {
            width: 620, max_results: 8,
            background: "#0a0010".into(), panel_background: "#0f0020".into(),
            text_color: "#e8e0f0".into(), text_dim: "#8888aa".into(),
            accent_color: "#c792ea".into(), selection_color: "#1a0035".into(),
            border_color: "#c792ea".into(), calculator: true, command_runner: true,
        }
    }
}
impl Default for LaunchCfg {
    fn default() -> Self { Self { launcher: Default::default() } }
}

// ── Keybind config ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bind {
    pub category: String,
    pub key:      String,
    pub action:   String,
    pub label:    String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindsCfg {
    #[serde(rename = "bind", default)]
    pub binds: Vec<Bind>,
}

// ── All configs ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AllConfigs {
    pub bar:     BarCfg,
    pub wall:    WallCfg,
    pub lock:    LockCfg,
    pub launch:  LaunchCfg,
    pub keybinds: KeybindsCfg,
}

impl AllConfigs {
    pub fn load() -> Self {
        Self {
            bar:      load_toml("bar.toml"),
            wall:     load_toml("wall.toml"),
            lock:     load_toml("lock.toml"),
            launch:   load_toml("launch.toml"),
            keybinds: load_keybinds(),
        }
    }

    pub fn save_bar(&self) -> Result<()> {
        write_toml("bar.toml", &self.bar)
    }

    pub fn save_wall(&self) -> Result<()> {
        write_toml("wall.toml", &self.wall)
    }

    pub fn save_lock(&self) -> Result<()> {
        write_toml("lock.toml", &self.lock)
    }

    pub fn save_launch(&self) -> Result<()> {
        write_toml("launch.toml", &self.launch)
    }

    pub fn save_keybinds(&self) -> Result<()> {
        write_toml_keybinds(&self.keybinds)?;
        write_sway_keybinds(&self.keybinds.binds)?;
        swaymsg_reload();
        Ok(())
    }
}

// ── Read/write helpers ────────────────────────────────────────────────────────

fn load_toml<T: for<'de> Deserialize<'de> + Default>(filename: &str) -> T {
    let path = cfg_dir().join(filename);
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => T::default(),
    }
}

fn load_keybinds() -> KeybindsCfg {
    let path = cfg_dir().join("keybinds.toml");
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => KeybindsCfg::default(),
    }
}

fn write_toml<T: Serialize>(filename: &str, val: &T) -> Result<()> {
    let path = cfg_dir().join(filename);
    let s = toml::to_string_pretty(val).context("serialize toml")?;
    std::fs::write(&path, s).with_context(|| format!("write {filename}"))
}

fn write_toml_keybinds(cfg: &KeybindsCfg) -> Result<()> {
    let path = cfg_dir().join("keybinds.toml");
    // Serialize as array of tables manually for clean output
    let mut out = String::from("# woven-cfg managed keybinds — source of truth\n");
    out.push_str("# Edit via woven-cfg. ~/.config/sway/woven-keybinds is auto-generated.\n\n");
    for b in &cfg.binds {
        out.push_str("[[bind]]\n");
        out.push_str(&format!("category = {:?}\n", b.category));
        out.push_str(&format!("key      = {:?}\n", b.key));
        out.push_str(&format!("action   = {:?}\n", b.action));
        out.push_str(&format!("label    = {:?}\n", b.label));
        out.push('\n');
    }
    std::fs::write(&path, out).context("write keybinds.toml")
}

pub fn write_sway_keybinds(binds: &[Bind]) -> Result<()> {
    use std::fmt::Write as FmtWrite;
    let mut out = String::new();
    writeln!(out, "# ── woven-keybinds ──────────────────────────────────────────────────────────────")?;
    writeln!(out, "# GENERATED by woven-cfg — do not edit manually")?;
    writeln!(out, "# Source of truth: ~/.config/woven-shell/keybinds.toml")?;
    writeln!(out, "# ─────────────────────────────────────────────────────────────────────────────────")?;
    writeln!(out)?;

    let categories = ["core", "apps", "woven", "focus", "move", "layout", "workspaces", "media", "screenshots"];
    let headers = ["Core", "Apps", "Woven shell", "Focus", "Move", "Layout", "Workspaces", "Media / System", "Screenshots"];

    for (cat, hdr) in categories.iter().zip(headers.iter()) {
        let group: Vec<_> = binds.iter().filter(|b| b.category == *cat).collect();
        if group.is_empty() { continue; }
        writeln!(out, "# ── {hdr} {}", "─".repeat(70 - hdr.len()))?;
        for b in &group {
            writeln!(out, "bindsym {} {}", b.key, b.action)?;
        }
        writeln!(out)?;
    }

    // Any other categories not in the fixed list
    let mut seen: std::collections::HashSet<&str> = categories.iter().copied().collect();
    for b in binds {
        if !seen.contains(b.category.as_str()) {
            seen.insert(&b.category);
            let group: Vec<_> = binds.iter().filter(|x| x.category == b.category).collect();
            writeln!(out, "# ── {} {}", b.category, "─".repeat(68usize.saturating_sub(b.category.len())))?;
            for gb in &group {
                writeln!(out, "bindsym {} {}", gb.key, gb.action)?;
            }
            writeln!(out)?;
        }
    }

    // Resize mode is always appended as-is
    writeln!(out, "# ── Resize mode ─────────────────────────────────────────────────────────────────")?;
    writeln!(out, "mode \"resize\" {{")?;
    writeln!(out, "    bindsym h      resize shrink width  10px")?;
    writeln!(out, "    bindsym j      resize grow   height 10px")?;
    writeln!(out, "    bindsym k      resize shrink height 10px")?;
    writeln!(out, "    bindsym l      resize grow   width  10px")?;
    writeln!(out, "    bindsym Left   resize shrink width  10px")?;
    writeln!(out, "    bindsym Down   resize grow   height 10px")?;
    writeln!(out, "    bindsym Up     resize shrink height 10px")?;
    writeln!(out, "    bindsym Right  resize grow   width  10px")?;
    writeln!(out, "    bindsym Return mode \"default\"")?;
    writeln!(out, "    bindsym Escape mode \"default\"")?;
    writeln!(out, "    bindsym $mod+r mode \"default\"")?;
    writeln!(out, "}}")?;
    writeln!(out, "bindsym $mod+r mode \"resize\"")?;

    std::fs::write(sway_keybinds_path(), out).context("write woven-keybinds")
}

fn swaymsg_reload() {
    let _ = std::process::Command::new("swaymsg").arg("reload").spawn();
}
