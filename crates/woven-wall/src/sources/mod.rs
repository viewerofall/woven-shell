//! Wallpaper source trait and factory.

mod color;
mod gradient;
mod static_img;
mod gif;
mod video;
mod slideshow;

use anyhow::Result;
use crate::config::WallpaperKind;

pub use color::ColorSource;
pub use gradient::GradientSource;
pub use static_img::ImageSource;
pub use gif::GifSource;
pub use video::VideoSource;
pub use slideshow::SlideshowSource;

pub trait Source: Send {
    /// Return BGRA (wl_shm Argb8888) pixel data for the given output size.
    fn frame(&mut self, width: u32, height: u32) -> Vec<u8>;
    /// Milliseconds to sleep after presenting a frame.
    fn frame_delay_ms(&self) -> u64;
    /// Handle an IPC command string. Default: no-op.
    fn handle_ipc(&mut self, _cmd: &str) {}
}

pub fn build(kind: &WallpaperKind) -> Result<Box<dyn Source>> {
    Ok(match kind {
        WallpaperKind::Color    { color }          => Box::new(ColorSource::new(color)),
        WallpaperKind::Gradient { colors, duration } => Box::new(GradientSource::new(colors, *duration)),
        WallpaperKind::Image    { path }            => Box::new(ImageSource::new(path)?),
        WallpaperKind::Gif      { path }            => Box::new(GifSource::new(path)?),
        WallpaperKind::Video    { path }            => Box::new(VideoSource::new(path)),
        WallpaperKind::Slideshow { dir, interval, transition, transition_secs, shuffle } =>
            Box::new(SlideshowSource::new(dir, *interval, transition.clone(), *transition_secs, *shuffle)?),
    })
}

/// Expand `~/` to the user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        format!("{home}/{rest}")
    } else {
        path.to_string()
    }
}

/// Scale-to-fill: resize so the image covers (w, h) then center-crop.
/// Returns BGRA bytes suitable for wl_shm Argb8888.
pub fn scale_to_fill_bgra(img: &image::DynamicImage, w: u32, h: u32, filter: image::imageops::FilterType) -> Vec<u8> {
    let scale_x = w as f32 / img.width()  as f32;
    let scale_y = h as f32 / img.height() as f32;
    let scale   = scale_x.max(scale_y);
    let new_w   = (img.width()  as f32 * scale).ceil() as u32;
    let new_h   = (img.height() as f32 * scale).ceil() as u32;

    let scaled  = img.resize(new_w, new_h, filter);
    let crop_x  = (new_w.saturating_sub(w)) / 2;
    let crop_y  = (new_h.saturating_sub(h)) / 2;
    let cropped = scaled.crop_imm(crop_x, crop_y, w, h);
    let rgba    = cropped.to_rgba8();

    let mut bgra = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        bgra.push(px[2]); // B
        bgra.push(px[1]); // G
        bgra.push(px[0]); // R
        bgra.push(px[3]); // A
    }
    bgra
}
