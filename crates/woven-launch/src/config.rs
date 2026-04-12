//! Launcher configuration — loaded from ~/.config/woven-shell/launch.toml

use serde::Deserialize;

#[derive(Deserialize)]
pub struct LaunchConfig {
    #[serde(default)]
    pub launcher: LauncherSettings,
}

#[derive(Deserialize)]
pub struct LauncherSettings {
    #[serde(default = "default_width")]
    pub width: u32,

    #[serde(default = "default_max_results")]
    pub max_results: usize,

    #[serde(default = "default_bg")]
    pub background: String,

    #[serde(default = "default_panel_bg")]
    pub panel_background: String,

    #[serde(default = "default_text")]
    pub text_color: String,

    #[serde(default = "default_text_dim")]
    pub text_dim: String,

    #[serde(default = "default_accent")]
    pub accent_color: String,

    #[serde(default = "default_selection")]
    pub selection_color: String,

    #[serde(default = "default_border")]
    pub border_color: String,

    #[serde(default = "default_true")]
    pub calculator: bool,

    #[serde(default = "default_true")]
    pub command_runner: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            width: 620,
            max_results: 8,
            background: "#0a0010".into(),
            panel_background: "#0f0020".into(),
            text_color: "#e8e0f0".into(),
            text_dim: "#8888aa".into(),
            accent_color: "#c792ea".into(),
            selection_color: "#1a0035".into(),
            border_color: "#c792ea".into(),
            calculator: true,
            command_runner: true,
        }
    }
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self { launcher: LauncherSettings::default() }
    }
}

impl LaunchConfig {
    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{home}/.config/woven-shell/launch.toml");
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}

fn default_width() -> u32 { 620 }
fn default_max_results() -> usize { 8 }
fn default_bg() -> String { "#0a0010".into() }
fn default_panel_bg() -> String { "#0f0020".into() }
fn default_text() -> String { "#e8e0f0".into() }
fn default_text_dim() -> String { "#8888aa".into() }
fn default_accent() -> String { "#c792ea".into() }
fn default_selection() -> String { "#1a0035".into() }
fn default_border() -> String { "#c792ea".into() }
fn default_true() -> bool { true }
