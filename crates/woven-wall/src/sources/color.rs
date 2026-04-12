//! Solid color wallpaper source.

use super::Source;

pub struct ColorSource {
    r: u8, g: u8, b: u8,
    cache: Vec<u8>,
    cache_size: (u32, u32),
}

impl ColorSource {
    pub fn new(hex: &str) -> Self {
        let (r, g, b) = parse_hex(hex);
        Self { r, g, b, cache: Vec::new(), cache_size: (0, 0) }
    }
}

impl Source for ColorSource {
    fn frame(&mut self, width: u32, height: u32) -> Vec<u8> {
        if self.cache_size != (width, height) {
            let n = (width * height) as usize;
            self.cache = (0..n).flat_map(|_| [self.b, self.g, self.r, 255]).collect();
            self.cache_size = (width, height);
        }
        self.cache.clone()
    }

    fn frame_delay_ms(&self) -> u64 { 1000 }
}

fn parse_hex(s: &str) -> (u8, u8, u8) {
    let s = s.trim_start_matches('#');
    if s.len() < 6 { return (0, 0, 0); }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    (r, g, b)
}
