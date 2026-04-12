//! Wallpaper loading and gaussian blur for lock screen background.
//! Completely independent from woven-wall — reads its own config.

use crate::config::BackgroundSettings;
use anyhow::{Context, Result};

/// Load wallpaper based on lock config, scale to (w,h), blur it.
/// Returns BGRA pixels.
pub fn load_blurred_wallpaper(bg: &BackgroundSettings, w: u32, h: u32, radius: u32) -> Result<Vec<u8>> {
    let home = std::env::var("HOME").unwrap_or_default();

    let path = match bg {
        BackgroundSettings::Image { path } => expand_tilde(path, &home),
        BackgroundSettings::Random { dir } => {
            let dir = expand_tilde(dir, &home);
            random_image_in_dir(&dir)?
        }
    };

    tracing::info!("lock: loading wallpaper from {path}");

    let img = image::open(&path)
        .with_context(|| format!("failed to open wallpaper: {path}"))?;

    // scale-to-fill (crop to aspect)
    let img = img.resize_to_fill(w, h, image::imageops::FilterType::Triangle);
    let rgba = img.to_rgba8();

    // convert to BGRA for wayland
    let mut bgra: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
    for pixel in rgba.pixels() {
        bgra.push(pixel[2]); // B
        bgra.push(pixel[1]); // G
        bgra.push(pixel[0]); // R
        bgra.push(pixel[3]); // A
    }

    // apply gaussian blur
    if radius > 0 {
        box_blur_bgra(&mut bgra, w, h, radius);
    }

    Ok(bgra)
}

/// Fallback: solid dark background if no wallpaper available
pub fn solid_background(w: u32, h: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    // #0a0010 — the TWM dark purple
    for chunk in buf.chunks_exact_mut(4) {
        chunk[0] = 0x10; // B
        chunk[1] = 0x00; // G
        chunk[2] = 0x0a; // R
        chunk[3] = 0xff; // A
    }
    buf
}

fn expand_tilde(path: &str, home: &str) -> String {
    if path.starts_with("~/") {
        format!("{home}{}", &path[1..])
    } else {
        path.to_string()
    }
}

fn random_image_in_dir(dir: &str) -> Result<String> {
    let entries: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .filter(|e| {
            let p = e.path();
            matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("jpg" | "jpeg" | "png" | "bmp" | "webp")
            )
        })
        .collect();

    if entries.is_empty() {
        anyhow::bail!("no images in {dir}");
    }

    // simple random: use current time nanos as seed
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    let idx = seed % entries.len();

    Ok(entries[idx].path().to_string_lossy().to_string())
}

/// Fast approximate gaussian blur via 3-pass box blur.
/// Operates on BGRA pixel buffer in-place.
fn box_blur_bgra(buf: &mut [u8], w: u32, h: u32, radius: u32) {
    let r = radius.max(1);
    // 3-pass box blur approximates gaussian well
    for _ in 0..3 {
        box_blur_h(buf, w, h, r);
        box_blur_v(buf, w, h, r);
    }
}

fn box_blur_h(buf: &mut [u8], w: u32, h: u32, r: u32) {
    let w = w as usize;
    let h = h as usize;
    let r = r as usize;
    let mut row = vec![0u8; w * 4];

    for y in 0..h {
        let base = y * w * 4;
        let mut sum = [0u32; 4];
        for x in 0..=r.min(w - 1) {
            let i = base + x * 4;
            for c in 0..4 { sum[c] += buf[i + c] as u32; }
        }
        let count_left = r.min(w - 1) + 1;
        for c in 0..4 { row[c] = (sum[c] / count_left as u32) as u8; }

        for x in 1..w {
            let right = (x + r).min(w - 1);
            let ri = base + right * 4;
            for c in 0..4 { sum[c] += buf[ri + c] as u32; }

            if x > r {
                let left = x - r - 1;
                let li = base + left * 4;
                for c in 0..4 { sum[c] -= buf[li + c] as u32; }
            }

            let left_edge = if x > r { x - r } else { 0 };
            let right_edge = (x + r).min(w - 1);
            let count = right_edge - left_edge + 1;
            let oi = x * 4;
            for c in 0..4 { row[oi + c] = (sum[c] / count as u32) as u8; }
        }

        buf[base..base + w * 4].copy_from_slice(&row);
    }
}

fn box_blur_v(buf: &mut [u8], w: u32, h: u32, r: u32) {
    let w = w as usize;
    let h = h as usize;
    let r = r as usize;
    let mut col = vec![0u8; h * 4];

    for x in 0..w {
        let mut sum = [0u32; 4];
        for y in 0..=r.min(h - 1) {
            let i = (y * w + x) * 4;
            for c in 0..4 { sum[c] += buf[i + c] as u32; }
        }
        let count_top = r.min(h - 1) + 1;
        for c in 0..4 { col[c] = (sum[c] / count_top as u32) as u8; }

        for y in 1..h {
            let bottom = (y + r).min(h - 1);
            let bi = (bottom * w + x) * 4;
            for c in 0..4 { sum[c] += buf[bi + c] as u32; }

            if y > r {
                let top = y - r - 1;
                let ti = (top * w + x) * 4;
                for c in 0..4 { sum[c] -= buf[ti + c] as u32; }
            }

            let top_edge = if y > r { y - r } else { 0 };
            let bottom_edge = (y + r).min(h - 1);
            let count = bottom_edge - top_edge + 1;
            let oi = y * 4;
            for c in 0..4 { col[oi + c] = (sum[c] / count as u32) as u8; }
        }

        for y in 0..h {
            let i = (y * w + x) * 4;
            let ci = y * 4;
            buf[i..i + 4].copy_from_slice(&col[ci..ci + 4]);
        }
    }
}
