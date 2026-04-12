//! Animated GIF wallpaper source.
//! Decodes all frames upfront, loops forever at the embedded frame delays.
//! Frames are scaled to each output size on demand and cached.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;
use anyhow::Result;
use image::{AnimationDecoder, DynamicImage, imageops::FilterType};
use image::codecs::gif::GifDecoder;
use super::{Source, expand_tilde, scale_to_fill_bgra};

struct RawFrame {
    delay_ms: u64,
    data:     image::RgbaImage,
}

pub struct GifSource {
    frames:       Vec<RawFrame>,
    current:      usize,
    last_advance: Instant,
    // (width, height, frame_index) → BGRA bytes
    cache:        HashMap<(u32, u32, usize), Vec<u8>>,
}

impl GifSource {
    pub fn new(path: &str) -> Result<Self> {
        let p    = expand_tilde(path);
        let file = File::open(&p)?;
        let dec  = GifDecoder::new(BufReader::new(file))?;
        let raw  = dec.into_frames().collect_frames()?;

        if raw.is_empty() { anyhow::bail!("gif has no frames: {p}"); }

        let frames = raw.into_iter().map(|f| {
            let (n, d) = f.delay().numer_denom_ms();
            let delay_ms = if d > 0 { ((n as u64) / (d as u64)).max(20) } else { 100 };
            RawFrame { delay_ms, data: f.into_buffer() }
        }).collect::<Vec<_>>();

        tracing::info!("wall: loaded gif — {} frames from {p}", frames.len());
        Ok(Self {
            frames,
            current:      0,
            last_advance: Instant::now(),
            cache:        HashMap::new(),
        })
    }

    fn tick(&mut self) {
        let due = self.frames[self.current].delay_ms;
        if self.last_advance.elapsed().as_millis() as u64 >= due {
            self.current     = (self.current + 1) % self.frames.len();
            self.last_advance = Instant::now();
        }
    }
}

impl Source for GifSource {
    fn frame(&mut self, width: u32, height: u32) -> Vec<u8> {
        self.tick();
        let idx   = self.current;
        let data  = &self.frames[idx].data;
        let cache = &mut self.cache;

        cache.entry((width, height, idx)).or_insert_with(|| {
            let img = DynamicImage::ImageRgba8(data.clone());
            scale_to_fill_bgra(&img, width, height, FilterType::Triangle)
        }).clone()
    }

    // Poll tightly so frame timing is accurate; actual pacing is in tick().
    fn frame_delay_ms(&self) -> u64 { 10 }
}
