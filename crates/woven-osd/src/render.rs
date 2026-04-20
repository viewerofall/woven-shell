//! OSD pill rendering — volume, brightness, media.

use tiny_skia::*;
use crate::state::{OsdKind, OsdState};
use crate::read::{MediaState, VolumeState};

pub const OSD_H: u32 = 56;
pub const OSD_W: u32 = 480;

const BG:     u32 = 0x0e0018;
const ACCENT: u32 = 0xc792ea;
const TEAL:   u32 = 0x00e5c8;
const FG:     u32 = 0xcdd6f4;
const DIM:    u32 = 0x4a3060;
const RED:    u32 = 0xf07178;
const BORDER: u32 = 0x2a1545;

pub fn render(state: &OsdState, font: &fontdue::Font) -> Vec<u8> {
    let mut pm = Pixmap::new(OSD_W, OSD_H).unwrap();
    // transparent background
    pm.fill(Color::TRANSPARENT);

    let Some(ref kind) = state.kind else {
        return bgra(pm);
    };

    let a = (state.alpha * 255.0) as u8;

    match kind {
        OsdKind::Volume(v)     => render_volume(&mut pm, font, v, a, state.offset_y),
        OsdKind::Brightness(b) => render_brightness(&mut pm, font, *b, a, state.offset_y),
        OsdKind::Media(m)      => render_media(&mut pm, font, m, a, state.offset_y),
    }

    bgra(pm)
}

// ── Volume ────────────────────────────────────────────────────────────────────

fn render_volume(pm: &mut Pixmap, font: &fontdue::Font,
                 v: &VolumeState, alpha: u8, off_y: f32) {
    let w   = OSD_W as f32;
    let pad = 18.0f32;
    let y   = off_y;

    // Pill background
    fill_pill(pm, 0.0, y, w, OSD_H as f32, alpha, BG, BORDER);

    // Icon
    let icon = if v.muted { "󰖁" } else if v.level > 60 { "󰕾" } else if v.level > 20 { "󰖀" } else { "󰕿" };
    blit(pm, font, icon, pad, y + 14.0, 22.0, if v.muted { RED } else { ACCENT }, alpha);

    // Device name
    let dev = truncate(font, &v.device, 12.0, 130.0);
    blit(pm, font, &dev, pad + 32.0, y + 11.0, 12.0, DIM, alpha);

    if v.muted {
        blit(pm, font, "MUTED", pad + 32.0, y + 28.0, 13.0, RED, alpha);
    } else {
        // Bar
        let bar_x = pad + 32.0 + 136.0;
        let bar_w = w - bar_x - 64.0;
        let fill  = bar_w * v.level as f32 / 100.0;
        fill_rrect_a(pm, bar_x, y + 22.0, bar_w, 8.0, 4.0, DIM, alpha);
        fill_rrect_a(pm, bar_x, y + 22.0, fill.max(4.0), 8.0, 4.0, TEAL, alpha);

        // Percentage
        let pct = format!("{}%", v.level);
        let pw  = measure(font, &pct, 13.0);
        blit(pm, font, &pct, w - pad - pw, y + 20.0, 13.0, FG, alpha);
    }
}

// ── Brightness ────────────────────────────────────────────────────────────────

fn render_brightness(pm: &mut Pixmap, font: &fontdue::Font,
                     level: u8, alpha: u8, off_y: f32) {
    let w   = OSD_W as f32;
    let pad = 18.0f32;
    let y   = off_y;

    fill_pill(pm, 0.0, y, w, OSD_H as f32, alpha, BG, BORDER);

    let icon = if level > 60 { "󰃠" } else if level > 20 { "󰃟" } else { "󰃞" };
    blit(pm, font, icon, pad, y + 14.0, 22.0, ACCENT, alpha);
    blit(pm, font, "Brightness", pad + 32.0, y + 11.0, 12.0, DIM, alpha);

    let bar_x = pad + 32.0 + 100.0;
    let bar_w = w - bar_x - 64.0;
    let fill  = bar_w * level as f32 / 100.0;
    fill_rrect_a(pm, bar_x, y + 22.0, bar_w, 8.0, 4.0, DIM, alpha);
    fill_rrect_a(pm, bar_x, y + 22.0, fill.max(4.0), 8.0, 4.0, ACCENT, alpha);

    let pct = format!("{level}%");
    let pw  = measure(font, &pct, 13.0);
    blit(pm, font, &pct, w - pad - pw, y + 20.0, 13.0, FG, alpha);
}

// ── Media ─────────────────────────────────────────────────────────────────────

fn render_media(pm: &mut Pixmap, font: &fontdue::Font,
                m: &MediaState, alpha: u8, off_y: f32) {
    let w   = OSD_W as f32;
    let pad = 18.0f32;
    let y   = off_y;

    fill_pill(pm, 0.0, y, w, OSD_H as f32, alpha, BG, BORDER);

    let icon = if m.playing { "󰐊" } else { "󰏤" };
    blit(pm, font, icon, pad, y + 14.0, 20.0, ACCENT, alpha);

    let avail  = w - pad * 2.0 - 32.0;
    let title  = truncate(font, &m.title, 14.0, avail * 0.55);
    let artist = truncate(font, &m.artist, 12.0, avail * 0.42);

    blit(pm, font, &title, pad + 32.0, y + 10.0, 14.0, FG, alpha);

    if !artist.is_empty() {
        blit(pm, font, "·", pad + 32.0 + measure(font, &title, 14.0) + 8.0, y + 12.0, 13.0, DIM, alpha);
        blit(pm, font, &artist, pad + 32.0 + measure(font, &title, 14.0) + 22.0, y + 12.0, 12.0, DIM, alpha);
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

fn fill_pill(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32,
             alpha: u8, bg: u32, border: u32) {
    fill_rrect_a(pm, x + 4.0, y + 4.0, w - 8.0, h - 8.0, (h - 8.0) / 2.0, bg, alpha);
    stroke_rrect_a(pm, x + 4.0, y + 4.0, w - 8.0, h - 8.0, (h - 8.0) / 2.0, border, alpha);
}

fn u32_to_color(rgb: u32, alpha: u8) -> Color {
    let r = ((rgb >> 16) & 0xFF) as u8;
    let g = ((rgb >> 8) & 0xFF) as u8;
    let b = (rgb & 0xFF) as u8;
    Color::from_rgba8(r, g, b, alpha)
}

fn paint_c(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 { return None; }
    let r = r.min(w / 2.0).min(h / 2.0);
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
    pb.finish()
}

fn fill_rrect_a(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, rgb: u32, alpha: u8) {
    if alpha == 0 { return; }
    let Some(path) = rrect_path(x, y, w, h, r) else { return };
    pm.fill_path(&path, &paint_c(u32_to_color(rgb, alpha)),
                 FillRule::Winding, Transform::identity(), None);
}

fn stroke_rrect_a(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, rgb: u32, alpha: u8) {
    if alpha == 0 { return; }
    let Some(path) = rrect_path(x + 0.5, y + 0.5, w - 1.0, h - 1.0, r) else { return };
    let mut stroke = Stroke::default();
    stroke.width = 1.0;
    pm.stroke_path(&path, &paint_c(u32_to_color(rgb, alpha)),
                   &stroke, Transform::identity(), None);
}

fn blit(pm: &mut Pixmap, font: &fontdue::Font,
        text: &str, x: f32, y: f32, size: f32, rgb: u32, alpha: u8) {
    if alpha == 0 { return; }
    let r = ((rgb >> 16) & 0xFF) as u8;
    let g = ((rgb >> 8) & 0xFF) as u8;
    let b = (rgb & 0xFF) as u8;

    let pw = pm.width()  as i32;
    let ph = pm.height() as i32;
    let mut cx = x;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        if metrics.width == 0 { cx += metrics.advance_width; continue; }
        let gx = (cx + metrics.xmin as f32).round() as i32;
        let gy = (y + size - metrics.height as f32 - metrics.ymin as f32).round() as i32;
        let pixels = pm.pixels_mut();
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + col];
                if coverage == 0 { continue; }
                let px = gx + col as i32;
                let py = gy + row as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
                let idx   = (py * pw + px) as usize;
                let dst   = &mut pixels[idx];
                let src_a = (coverage as u16 * alpha as u16 / 255) as u8;
                let inv_a = 255u16 - src_a as u16;
                let dr = (r as u16 * src_a as u16 / 255 + dst.red()   as u16 * inv_a / 255) as u8;
                let dg = (g as u16 * src_a as u16 / 255 + dst.green() as u16 * inv_a / 255) as u8;
                let db = (b as u16 * src_a as u16 / 255 + dst.blue()  as u16 * inv_a / 255) as u8;
                let da = (src_a as u16 + dst.alpha() as u16 * inv_a / 255).min(255) as u8;
                *dst = PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
            }
        }
        cx += metrics.advance_width;
    }
}

fn measure(font: &fontdue::Font, text: &str, size: f32) -> f32 {
    text.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

fn truncate(font: &fontdue::Font, s: &str, size: f32, max_w: f32) -> String {
    let ellipsis_w = measure(font, "…", size);
    let mut out = String::new();
    let mut w   = 0.0f32;
    for ch in s.chars() {
        let cw = font.metrics(ch, size).advance_width;
        if w + cw + ellipsis_w > max_w && !out.is_empty() {
            out.push('…');
            return out;
        }
        out.push(ch);
        w += cw;
    }
    out
}

fn bgra(pm: Pixmap) -> Vec<u8> {
    let data = pm.data();
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        out.push(chunk[2]);
        out.push(chunk[1]);
        out.push(chunk[0]);
        out.push(chunk[3]);
    }
    out
}
