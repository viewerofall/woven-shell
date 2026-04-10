//! tiny-skia drawing helpers for woven-bar.
//! Wraps common bar painting ops so widgets stay clean.

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

/// Parse a hex color string (#RRGGBB or #AARRGGBB) into a tiny-skia Color.
/// Alpha defaults to 1.0 if not supplied.
pub fn hex_color(s: &str) -> Color {
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, 255)
        }
        8 => {
            let a = u8::from_str_radix(&s[0..2], 16).unwrap_or(255);
            let r = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[6..8], 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, a)
        }
        _ => Color::BLACK,
    }
}

/// Fill the entire pixmap with a solid color.
pub fn clear(pixmap: &mut Pixmap, color: Color) {
    pixmap.fill(color);
}

/// Fill a solid rectangle.
pub fn fill_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    let Some(rect) = Rect::from_xywh(x, y, w, h) else { return; };
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = false;
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

/// Fill a rounded rectangle. Falls back to plain rect when radius ≤ 0 or shape is tiny.
pub fn fill_rounded_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    if r <= 0.0 || w < r * 2.0 || h < r * 2.0 {
        fill_rect(pixmap, x, y, w, h, color);
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();

    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Draw a filled circle.
pub fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color) {
    if r <= 0.0 { return; }
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Blit pre-decoded RGBA icon pixels into `pixmap` at (x, y), scaled to `size×size`.
/// Converts RGBA → premultiplied ARGB8888 during blit.
pub fn blit_icon(
    pixmap: &mut Pixmap,
    x: i32,
    y: i32,
    size: u32,
    rgba: &[u8],
    src_w: u32,
    src_h: u32,
) {
    let pw = pixmap.width()  as i32;
    let ph = pixmap.height() as i32;
    let pixels = pixmap.pixels_mut();

    for dy in 0..size as i32 {
        for dx in 0..size as i32 {
            let px = x + dx;
            let py = y + dy;
            if px < 0 || py < 0 || px >= pw || py >= ph { continue; }

            let sx = (dx as f32 / size as f32 * src_w as f32) as usize;
            let sy = (dy as f32 / size as f32 * src_h as f32) as usize;
            let si = (sy * src_w as usize + sx) * 4;
            if si + 3 >= rgba.len() { continue; }

            let sr = rgba[si];
            let sg = rgba[si + 1];
            let sb = rgba[si + 2];
            let sa = rgba[si + 3];
            if sa == 0 { continue; }

            let idx  = (py * pw + px) as usize;
            let dst  = &mut pixels[idx];
            let src_a = sa as u16;
            let inv_a = 255u16 - src_a;
            let dr = ((sr as u16 * src_a + dst.red()   as u16 * inv_a) / 255) as u8;
            let dg = ((sg as u16 * src_a + dst.green() as u16 * inv_a) / 255) as u8;
            let db = ((sb as u16 * src_a + dst.blue()  as u16 * inv_a) / 255) as u8;
            let da = src_a.saturating_add(dst.alpha() as u16 * inv_a / 255) as u8;
            *dst = tiny_skia::PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
        }
    }
}
