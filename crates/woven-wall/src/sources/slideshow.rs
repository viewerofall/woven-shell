//! Slideshow wallpaper source — cycles through a directory of images
//! with animated transitions between slides.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use image::imageops::FilterType;
use image::DynamicImage;

use crate::config::TransitionKind;
use super::{Source, expand_tilde, scale_to_fill_bgra};

// ─── transition functions ─────────────────────────────────────────────────────

/// Blend `prev` → `next` linearly.
fn fade(prev: &[u8], next: &[u8], t: f32) -> Vec<u8> {
    prev.iter().zip(next.iter())
        .map(|(&p, &n)| (p as f32 + (n as f32 - p as f32) * t) as u8)
        .collect()
}

/// Hard-edge left-to-right wipe: next image reveals from left.
fn wipe(prev: &[u8], next: &[u8], t: f32, w: u32, h: u32) -> Vec<u8> {
    let edge = (w as f32 * t) as u32;
    let mut out = vec![0u8; prev.len()];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let src = if x < edge { next } else { prev };
            out[i..i + 4].copy_from_slice(&src[i..i + 4]);
        }
    }
    out
}

/// Next image slides in from the right, prev stays in place.
fn slide(prev: &[u8], next: &[u8], t: f32, w: u32, h: u32) -> Vec<u8> {
    // offset = how many columns of next are visible from the right
    let visible = (w as f32 * t) as u32;
    let mut out = vec![0u8; prev.len()];
    for y in 0..h {
        let prev_row_start = (y * w) as usize * 4;
        let next_row_start = (y * w) as usize * 4;

        // prev occupies x in [0, w-visible)
        let prev_cols = w.saturating_sub(visible) as usize;
        let dst = prev_row_start;
        let src = prev_row_start;
        out[dst..dst + prev_cols * 4].copy_from_slice(&prev[src..src + prev_cols * 4]);

        // next occupies x in [w-visible, w), sourced from next at x in [0, visible)
        let next_start_dst = prev_row_start + prev_cols * 4;
        let next_start_src = next_row_start;
        let next_cols = visible as usize;
        if next_cols > 0 {
            out[next_start_dst..next_start_dst + next_cols * 4]
                .copy_from_slice(&next[next_start_src..next_start_src + next_cols * 4]);
        }
    }
    out
}

/// Pixelate-out current → pixelate-in next.
/// t ∈ [0, 0.5]: block size grows (pixelating prev)
/// t ∈ [0.5, 1]: block size shrinks back (revealing next)
fn pixelate(prev: &[u8], next: &[u8], t: f32, w: u32, h: u32) -> Vec<u8> {
    const MAX_BLOCK: u32 = 64;

    let (source, phase_t) = if t < 0.5 {
        (prev, t / 0.5)
    } else {
        (next, 1.0 - (t - 0.5) / 0.5)
    };

    let block_size = (1.0 + (MAX_BLOCK as f32 - 1.0) * ease_in_out(phase_t)).round() as u32;
    let block_size = block_size.max(1);

    let mut out = vec![0u8; (w * h * 4) as usize];

    let blocks_x = w.div_ceil(block_size);
    let blocks_y = h.div_ceil(block_size);

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let x0 = bx * block_size;
            let y0 = by * block_size;
            let x1 = (x0 + block_size).min(w);
            let y1 = (y0 + block_size).min(h);

            // average color of this block from source
            let (mut sr, mut sg, mut sb) = (0u32, 0u32, 0u32);
            let mut count = 0u32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let i = ((py * w + px) * 4) as usize;
                    sb += source[i]     as u32;
                    sg += source[i + 1] as u32;
                    sr += source[i + 2] as u32;
                    count += 1;
                }
            }
            if count == 0 { continue; }
            let (ab, ag, ar) = (
                (sb / count) as u8,
                (sg / count) as u8,
                (sr / count) as u8,
            );

            // fill block with averaged color
            for py in y0..y1 {
                for px in x0..x1 {
                    let i = ((py * w + px) * 4) as usize;
                    out[i]     = ab;
                    out[i + 1] = ag;
                    out[i + 2] = ar;
                    out[i + 3] = 255;
                }
            }
        }
    }
    out
}

fn ease_in_out(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

// ─── SlideshowSource ─────────────────────────────────────────────────────────

struct CachedImage {
    img:   DynamicImage,
    cache: HashMap<(u32, u32), Vec<u8>>,
}

impl CachedImage {
    fn new(img: DynamicImage) -> Self { Self { img, cache: HashMap::new() } }

    fn bgra(&mut self, w: u32, h: u32) -> &Vec<u8> {
        let img = &self.img;
        self.cache.entry((w, h)).or_insert_with(|| {
            scale_to_fill_bgra(img, w, h, FilterType::Lanczos3)
        })
    }
}

pub struct SlideshowSource {
    images:     Vec<PathBuf>,
    index:      usize,
    order:      Vec<usize>,    // playback order indices into `images`
    order_pos:  usize,

    current:    CachedImage,
    next:       Option<CachedImage>,

    interval_ms:     u64,
    transition:      TransitionKind,
    transition_ms:   u64,
    last_shown:      Instant,
    transition_start: Option<Instant>,
    changed:         bool,
}

impl SlideshowSource {
    pub fn new(
        dir: &str,
        interval_secs: u64,
        transition: TransitionKind,
        transition_secs: f64,
        shuffle: bool,
    ) -> Result<Self> {
        let dir_path = expand_tilde(dir);
        let images   = collect_images(&dir_path)?;

        if images.is_empty() {
            anyhow::bail!("slideshow: no images found in {dir_path}");
        }

        // start at default.png if present, otherwise index 0
        let start_idx = images.iter()
            .position(|p| p.file_name().map(|n| n == "default.png").unwrap_or(false))
            .unwrap_or(0);

        let mut order: Vec<usize> = (0..images.len()).collect();
        if shuffle {
            // Fisher-Yates using timestamp entropy — no rand dep needed
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(42) as usize;
            for i in (1..order.len()).rev() {
                let j = (seed.wrapping_mul(i + 1).wrapping_add(i)) % (i + 1);
                order.swap(i, j);
            }
        }

        // put start_idx first in order
        let order_start = order.iter().position(|&x| x == start_idx).unwrap_or(0);
        order.rotate_left(order_start);

        let first_path = &images[order[0]];
        tracing::info!("slideshow: starting with {}", first_path.display());
        let first_img = image::open(first_path)?;

        Ok(Self {
            images,
            index: order[0],
            order,
            order_pos: 0,

            current: CachedImage::new(first_img),
            next:    None,

            interval_ms:      interval_secs * 1000,
            transition,
            transition_ms:    (transition_secs * 1000.0) as u64,
            last_shown:       Instant::now(),
            transition_start: None,
            changed:          true,
        })
    }

    fn advance_index(&mut self) {
        self.order_pos = (self.order_pos + 1) % self.order.len();
        self.index     = self.order[self.order_pos];
    }

    fn load_next(&mut self) {
        let mut peek_pos = (self.order_pos + 1) % self.order.len();
        // skip the current index if there's only one image
        if self.order.len() == 1 { peek_pos = 0; }
        let next_idx  = self.order[peek_pos];
        let next_path = &self.images[next_idx];
        tracing::info!("slideshow: loading next → {}", next_path.display());
        match image::open(next_path) {
            Ok(img) => { self.next = Some(CachedImage::new(img)); }
            Err(e)  => { tracing::warn!("slideshow: failed to load {}: {e}", next_path.display()); }
        }
    }

    fn finish_transition(&mut self) {
        if let Some(next) = self.next.take() {
            self.advance_index();
            self.current          = next;
            self.transition_start = None;
            self.last_shown       = Instant::now();
            self.changed          = true;
            tracing::debug!("slideshow: transitioned to index {}", self.index);
        }
    }
}

impl SlideshowSource {
    /// Force an immediate transition to the next image.
    pub fn force_next(&mut self) {
        // abort any in-progress transition cleanly
        if let Some(next) = self.next.take() {
            self.advance_index();
            self.current = next;
        }
        self.transition_start = None;
        self.last_shown = Instant::now() - std::time::Duration::from_millis(self.interval_ms + 1);
    }

    /// Force an immediate transition to the previous image.
    pub fn force_prev(&mut self) {
        if let Some(next) = self.next.take() { drop(next); }
        self.transition_start = None;

        let len = self.order.len();
        self.order_pos = (self.order_pos + len - 1) % len;
        // go one more back so force_next lands on what we want
        self.order_pos = (self.order_pos + len - 1) % len;
        self.index = self.order[self.order_pos];
        self.last_shown = Instant::now() - std::time::Duration::from_millis(self.interval_ms + 1);
    }

    /// Force an immediate transition to a specific image path.
    pub fn force_set(&mut self, path: &str) {
        if let Some(next) = self.next.take() { drop(next); }
        self.transition_start = None;
        match image::open(path) {
            Ok(img) => {
                // Update order_pos so slideshow continues from this image
                let path_buf = PathBuf::from(path);
                if let Some(img_idx) = self.images.iter().position(|p| p == &path_buf) {
                    if let Some(pos) = self.order.iter().position(|&x| x == img_idx) {
                        // Set one before so finish_transition's advance_index() lands here
                        let len = self.order.len();
                        self.order_pos = (pos + len - 1) % len;
                    }
                }
                self.next = Some(CachedImage::new(img));
                self.transition_start = Some(Instant::now());
                tracing::info!("slideshow: IPC set → {path}");
            }
            Err(e) => tracing::warn!("slideshow: IPC set failed for {path}: {e}"),
        }
    }
}

impl Source for SlideshowSource {
    fn handle_ipc(&mut self, cmd: &str) {
        match cmd.trim() {
            "next" => self.force_next(),
            "prev" => self.force_prev(),
            s if s.starts_with("set ") => {
                let path = expand_tilde(s["set ".len()..].trim());
                self.force_set(&path);
            }
            _ => {}
        }
    }

    fn frame(&mut self, w: u32, h: u32) -> Vec<u8> {
        let elapsed_ms = self.last_shown.elapsed().as_millis() as u64;

        // time to start a transition?
        if self.transition_start.is_none() && self.next.is_none() && elapsed_ms >= self.interval_ms {
            self.load_next();
            if self.next.is_some() {
                self.transition_start = Some(Instant::now());
            }
        }

        match &self.transition_start {
            None => {
                // idle — serve current image
                self.current.bgra(w, h).clone()
            }
            Some(ts) => {
                let elapsed_trans = ts.elapsed().as_millis() as u64;
                let t = (elapsed_trans as f32 / self.transition_ms as f32).clamp(0.0, 1.0);

                // make sure next is loaded
                if self.next.is_none() {
                    // fallback: transition already started but next failed, just finish
                    self.transition_start = None;
                    self.last_shown = Instant::now();
                    return self.current.bgra(w, h).clone();
                }

                let prev = self.current.bgra(w, h).clone();
                let next = self.next.as_mut().unwrap().bgra(w, h).clone();

                let frame = match self.transition {
                    TransitionKind::Fade     => fade(&prev, &next, t),
                    TransitionKind::Wipe     => wipe(&prev, &next, t, w, h),
                    TransitionKind::Slide    => slide(&prev, &next, t, w, h),
                    TransitionKind::Pixelate => pixelate(&prev, &next, t, w, h),
                };

                if t >= 1.0 {
                    self.finish_transition();
                }

                frame
            }
        }
    }

    fn frame_delay_ms(&self) -> u64 {
        if self.transition_start.is_some() { 33 } else { 500 }
    }

    fn wallpaper_changed(&mut self) -> bool {
        if self.changed { self.changed = false; true } else { false }
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn collect_images(dir: &str) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file() && matches!(
                p.extension().and_then(|x| x.to_str()).map(|x| x.to_lowercase()).as_deref(),
                Some("png" | "jpg" | "jpeg" | "webp")
            )
        })
        .collect();
    paths.sort();
    Ok(paths)
}
