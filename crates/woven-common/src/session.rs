use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

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

pub struct SessionClient {
    stream: Option<UnixStream>,
}

impl SessionClient {
    pub fn new() -> Self {
        let stream = UnixStream::connect("/tmp/woven-session.sock")
            .ok()
            .and_then(|s| {
                s.set_read_timeout(Some(Duration::from_secs(1))).ok();
                s.set_write_timeout(Some(Duration::from_secs(1))).ok();
                Some(s)
            });

        SessionClient { stream }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn get_battery(&mut self) -> Option<BatteryState> {
        self.query("get_battery")
    }

    pub fn get_media(&mut self) -> Option<MediaState> {
        self.query("get_media")
    }

    pub fn get_power(&mut self) -> Option<PowerState> {
        self.query("get_power")
    }

    fn query<T: serde::de::DeserializeOwned>(&mut self, cmd: &str) -> Option<T> {
        let stream = self.stream.as_mut()?;

        stream.write_all(format!("{}\n", cmd).as_bytes()).ok()?;
        stream.flush().ok()?;

        let mut reader = BufReader::new(stream.try_clone().ok()?);
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;

        serde_json::from_str(&line).ok()
    }
}

impl Default for SessionClient {
    fn default() -> Self {
        Self::new()
    }
}
