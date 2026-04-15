//! Static image wallpaper source (PNG, JPG).
//! Scales to fill on first frame, caches per output size.

use std::collections::HashMap;
use anyhow::Result;
use image::imageops::FilterType;
use super::{Source, expand_tilde, scale_to_fill_bgra};


pub struct ImageSource {
    img:     image::DynamicImage,
    cache:   HashMap<(u32, u32), Vec<u8>>,
    changed: bool,
}

impl ImageSource {
    pub fn new(path: &str) -> Result<Self> {
        let p = expand_tilde(path);
        let img = image::open(&p)?;
        tracing::info!("wall: loaded image from {p}");
        Ok(Self { img, cache: HashMap::new(), changed: true })
    }
}

impl Source for ImageSource {
    fn frame(&mut self, width: u32, height: u32) -> Vec<u8> {
        let img = &self.img;
        self.cache
            .entry((width, height))
            .or_insert_with(|| {
                tracing::debug!("wall: scaling image to {width}×{height}");
                scale_to_fill_bgra(img, width, height, FilterType::Lanczos3)
            })
            .clone()
    }

    fn frame_delay_ms(&self) -> u64 { 1000 }

    fn handle_ipc(&mut self, cmd: &str) {
        if let Some(path) = cmd.strip_prefix("set ") {
            let p = expand_tilde(path.trim());
            match image::open(&p) {
                Ok(img) => {
                    self.img = img;
                    self.cache.clear();
                    self.changed = true;
                    tracing::info!("wall: IPC set → loaded {p}");
                }
                Err(e) => tracing::warn!("wall: IPC set failed for {p}: {e}"),
            }
        }
    }

    fn wallpaper_changed(&mut self) -> bool {
        if self.changed { self.changed = false; true } else { false }
    }
}
