//! Animated diagonal gradient wallpaper source.
//! Cycles through a color array, drawing a top-left → bottom-right gradient
//! where both endpoints animate independently (offset by half a cycle) so the
//! gradient itself shifts and flows rather than just cross-fading.

use std::time::Instant;
use tiny_skia::{Color, GradientStop, LinearGradient, Paint, Pixmap, Point, Rect, SpreadMode, Transform};
use super::Source;

pub struct GradientSource {
    colors:   Vec<(u8, u8, u8)>,
    duration: f64,
    start:    Instant,
}

impl GradientSource {
    pub fn new(hex_colors: &[String], duration: f64) -> Self {
        let colors = hex_colors.iter().map(|s| parse_hex(s)).collect();
        Self { colors, duration, start: Instant::now() }
    }

    /// Sample the color array at a fractional position in [0, ∞), wrapping.
    fn sample(&self, phase: f64) -> (u8, u8, u8) {
        let n = self.colors.len();
        if n == 0 { return (0, 0, 0); }
        if n == 1 { return self.colors[0]; }

        let t   = phase.fract();
        let ft  = t * (n - 1) as f64;
        let i   = (ft as usize).min(n - 2);
        let u   = ft - i as f64;
        let (r0, g0, b0) = self.colors[i];
        let (r1, g1, b1) = self.colors[i + 1];
        let l = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * u) as u8;
        (l(r0, r1), l(g0, g1), l(b0, b1))
    }
}

impl Source for GradientSource {
    fn frame(&mut self, width: u32, height: u32) -> Vec<u8> {
        let t  = self.start.elapsed().as_secs_f64() / self.duration;
        let ca = self.sample(t);
        let cb = self.sample(t + 0.5); // offset endpoint so the gradient flows

        let mut pixmap = match Pixmap::new(width, height) {
            Some(p) => p,
            None    => return vec![0u8; (width * height * 4) as usize],
        };

        if let (Some(rect), Some(shader)) = (
            Rect::from_xywh(0.0, 0.0, width as f32, height as f32),
            LinearGradient::new(
                Point::from_xy(0.0, 0.0),
                Point::from_xy(width as f32, height as f32),
                vec![
                    GradientStop::new(0.0, Color::from_rgba8(ca.0, ca.1, ca.2, 255)),
                    GradientStop::new(1.0, Color::from_rgba8(cb.0, cb.1, cb.2, 255)),
                ],
                SpreadMode::Pad,
                Transform::identity(),
            ),
        ) {
            let mut paint = Paint::default();
            paint.shader = shader;
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }

        // tiny-skia premultiplied RGBA → wl_shm BGRA
        let data = pixmap.data();
        let mut out = Vec::with_capacity(data.len());
        for px in data.chunks_exact(4) {
            out.push(px[2]); // B
            out.push(px[1]); // G
            out.push(px[0]); // R
            out.push(px[3]); // A
        }
        out
    }

    fn frame_delay_ms(&self) -> u64 { 50 } // 20 fps — plenty for a gradient
}

fn parse_hex(s: &str) -> (u8, u8, u8) {
    let s = s.trim_start_matches('#');
    if s.len() < 6 { return (0, 0, 0); }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    (r, g, b)
}
