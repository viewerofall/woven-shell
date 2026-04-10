//! woven-bar configuration.
//! Loaded from ~/.config/woven-shell/bar.toml (TOML for now, Lua later).

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarConfig {
    #[serde(default = "default_height")]
    pub height: u32,

    #[serde(default)]
    pub position: BarPosition,

    #[serde(default)]
    pub theme: Theme,

    #[serde(default)]
    pub modules: Modules,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            height:   32,
            position: BarPosition::Top,
            theme:    Theme::default(),
            modules:  Modules::default(),
        }
    }
}

impl BarConfig {
    pub fn load() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        let path = format!("{home}/.config/woven-shell/bar.toml");

        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(toml::from_str(&s).unwrap_or_else(|e| {
                tracing::warn!("bar.toml parse error: {e} — using defaults");
                Self::default()
            })),
            Err(_) => {
                tracing::info!("no bar.toml found at {path}, using defaults");
                Ok(Self::default())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BarPosition {
    #[default]
    Top,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Bar background color (hex)
    pub background: String,
    /// Primary text color (hex)
    pub foreground: String,
    /// Accent / active highlight color (hex)
    pub accent: String,
    /// Dimmed text / inactive color (hex)
    pub dim: String,
    /// Border radius for pill widgets (px)
    pub radius: u32,
    /// Font size in logical pixels
    pub font_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: "#0a0010".into(),
            foreground: "#cdd6f4".into(),
            accent:     "#c792ea".into(),
            dim:        "#4a3060".into(),
            radius:     6,
            font_size:  12.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modules {
    pub left:   Vec<ModuleKind>,
    pub center: Vec<ModuleKind>,
    pub right:  Vec<ModuleKind>,
}

impl Default for Modules {
    fn default() -> Self {
        Self {
            left: vec![
                ModuleKind::Activities,
                ModuleKind::Workspaces,
            ],
            center: vec![
                ModuleKind::WindowTitle,
            ],
            right: vec![
                ModuleKind::Network,
                ModuleKind::Audio,
                ModuleKind::Battery,
                ModuleKind::Clock,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    Activities,
    Workspaces,
    WindowTitle,
    Network,
    Audio,
    Battery,
    Clock,
    Systray,
    Cpu,
    Memory,
    Disk,
    Temp,
    Media,
    Notifications,
    ControlCenter,
}

fn default_height() -> u32 { 32 }
