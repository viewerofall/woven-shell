use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryState {
    pub percent: u8,
    pub ac_online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaState {
    pub playing: bool,
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerState {
    pub can_suspend: bool,
    pub can_poweroff: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum SessionEvent {
    #[serde(rename = "battery_changed")]
    BatteryChanged(BatteryState),
    #[serde(rename = "media_changed")]
    MediaChanged(MediaState),
    #[serde(rename = "power_changed")]
    PowerChanged(PowerState),
}

pub struct SessionState {
    pub battery: Arc<RwLock<BatteryState>>,
    pub media: Arc<RwLock<MediaState>>,
    pub power: Arc<RwLock<PowerState>>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            battery: Arc::new(RwLock::new(BatteryState {
                percent: 0,
                ac_online: false,
            })),
            media: Arc::new(RwLock::new(MediaState {
                playing: false,
                title: String::new(),
                artist: String::new(),
            })),
            power: Arc::new(RwLock::new(PowerState {
                can_suspend: true,
                can_poweroff: true,
            })),
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}
