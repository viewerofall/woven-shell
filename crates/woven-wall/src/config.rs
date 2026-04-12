//! woven-wall configuration.
//! Loaded from ~/.config/woven-shell/wall.toml

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WallConfig {
    pub wallpaper: WallpaperKind,
}

impl WallConfig {
    pub fn load() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        let path = format!("{home}/.config/woven-shell/wall.toml");

        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(toml::from_str(&s).unwrap_or_else(|e| {
                tracing::warn!("wall.toml parse error: {e} — using slideshow default");
                Self::default()
            })),
            Err(_) => {
                tracing::info!("no wall.toml at {path} — using slideshow default");
                Ok(Self::default())
            }
        }
    }
}

impl Default for WallConfig {
    fn default() -> Self {
        Self { wallpaper: WallpaperKind::default() }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransitionKind {
    #[default]
    Pixelate,
    Fade,
    Wipe,
    Slide,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WallpaperKind {
    /// Solid fill
    Color {
        #[serde(default = "default_color")]
        color: String,
    },
    /// Animated diagonal gradient cycling through a color array
    Gradient {
        #[serde(default = "default_gradient_colors")]
        colors: Vec<String>,
        /// Seconds for one full animation cycle
        #[serde(default = "default_duration")]
        duration: f64,
    },
    /// Static PNG / JPG — scaled to fill
    Image {
        path: String,
    },
    /// Animated GIF — scaled to fill, loops forever
    Gif {
        path: String,
    },
    /// Video (mp4, mkv, …) via ffmpeg — loops forever
    Video {
        path: String,
    },
    /// Slideshow — cycles images from a directory with transitions
    Slideshow {
        /// Directory containing images (PNG, JPG, JPEG, WEBP)
        #[serde(default = "default_slideshow_dir")]
        dir: String,
        /// Seconds each image is shown before transitioning
        #[serde(default = "default_interval")]
        interval: u64,
        /// Transition effect between slides
        #[serde(default)]
        transition: TransitionKind,
        /// Duration of the transition animation in seconds
        #[serde(default = "default_transition_secs")]
        transition_secs: f64,
        /// Shuffle order instead of alphabetical
        #[serde(default)]
        shuffle: bool,
    },
}

impl Default for WallpaperKind {
    fn default() -> Self {
        Self::Slideshow {
            dir:             default_slideshow_dir(),
            interval:        default_interval(),
            transition:      TransitionKind::Pixelate,
            transition_secs: default_transition_secs(),
            shuffle:         false,
        }
    }
}

fn default_color() -> String { "#0a0010".into() }
fn default_gradient_colors() -> Vec<String> {
    vec!["#0a0010".into(), "#1a0030".into(), "#0a0020".into(), "#0d001a".into()]
}
fn default_duration() -> f64 { 30.0 }
fn default_slideshow_dir() -> String { "~/Pictures/Wallpapers".into() }
fn default_interval() -> u64 { 300 }
fn default_transition_secs() -> f64 { 1.5 }
