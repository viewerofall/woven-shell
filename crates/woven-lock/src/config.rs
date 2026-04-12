//! Lock screen configuration — loaded from ~/.config/woven-shell/lock.toml

use serde::Deserialize;

#[derive(Deserialize)]
pub struct LockConfig {
    #[serde(default)]
    pub lock: LockSettings,

    #[serde(default)]
    pub background: BackgroundSettings,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type")]
pub enum BackgroundSettings {
    /// Single image: background.type = "image", background.path = "..."
    #[serde(rename = "image")]
    Image { path: String },

    /// Random image from a directory each time lock runs
    #[serde(rename = "random")]
    Random { dir: String },
}

impl Default for BackgroundSettings {
    fn default() -> Self {
        BackgroundSettings::Random { dir: "~/Pictures/Wallpapers".into() }
    }
}

#[derive(Deserialize)]
pub struct LockSettings {
    #[serde(default = "default_blur_radius")]
    pub blur_radius: u32,

    #[serde(default = "default_true")]
    pub show_clock: bool,

    #[serde(default = "default_clock_format")]
    pub clock_format: String,

    #[serde(default = "default_true")]
    pub show_date: bool,

    #[serde(default = "default_date_format")]
    pub date_format: String,

    #[serde(default = "default_text_color")]
    pub text_color: String,

    #[serde(default = "default_accent_color")]
    pub accent_color: String,

    #[serde(default = "default_error_color")]
    pub error_color: String,

    #[serde(default = "default_fade_ms")]
    pub fade_in_ms: u32,

    #[serde(default = "default_fade_ms")]
    pub fade_out_ms: u32,

    #[serde(default = "default_true")]
    pub shake_on_error: bool,
}

impl Default for LockSettings {
    fn default() -> Self {
        Self {
            blur_radius:    20,
            show_clock:     true,
            clock_format:   "%H:%M".into(),
            show_date:      true,
            date_format:    "%A, %B %e".into(),
            text_color:     "#cdd6f4".into(),
            accent_color:   "#cba6f7".into(),
            error_color:    "#f07178".into(),
            fade_in_ms:     200,
            fade_out_ms:    200,
            shake_on_error: true,
        }
    }
}

impl LockConfig {
    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{home}/.config/woven-shell/lock.toml");
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}

impl Default for LockConfig {
    fn default() -> Self {
        Self { lock: LockSettings::default(), background: BackgroundSettings::default() }
    }
}

fn default_blur_radius() -> u32 { 20 }
fn default_true() -> bool { true }
fn default_clock_format() -> String { "%H:%M".into() }
fn default_date_format() -> String { "%A, %B %e".into() }
fn default_text_color() -> String { "#cdd6f4".into() }
fn default_accent_color() -> String { "#cba6f7".into() }
fn default_error_color() -> String { "#f07178".into() }
fn default_fade_ms() -> u32 { 200 }
