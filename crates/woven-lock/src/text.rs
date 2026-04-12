//! Text renderer — identical to woven-bar's, reused here.

use fontdue::{Font, FontSettings, layout::{CoordinateSystem, Layout, TextStyle}};
use std::collections::HashMap;
use tiny_skia::{Color, Pixmap};

#[derive(Hash, PartialEq, Eq, Clone)]
struct GlyphKey(u8, char, u16);

struct CachedGlyph { width: usize, height: usize, bitmap: Vec<u8> }

#[derive(Hash, PartialEq, Eq, Clone)]
struct MeasureKey(Box<str>, u16);

struct LayoutEntry { glyphs: Vec<(GlyphKey, f32, f32)>, advance: f32 }

pub struct TextRenderer {
    fonts:         Vec<Font>,
    layout:        Layout,
    has_nerd_font: bool,
    glyph_cache:   HashMap<GlyphKey, CachedGlyph>,
    layout_cache:  HashMap<MeasureKey, LayoutEntry>,
    measure_cache: HashMap<MeasureKey, f32>,
}

impl Default for TextRenderer { fn default() -> Self { Self::new() } }

impl TextRenderer {
    pub fn new() -> Self {
        let text_data = load_font(&[
            "/usr/share/fonts/TTF/Inconsolata-Regular.ttf",
            "/usr/share/fonts/TTF/Inconsolata.ttf",
            "/usr/share/fonts/truetype/inconsolata/Inconsolata-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]).unwrap_or_else(|| include_bytes!("../../woven-bar/fonts/NotoSans-Regular.ttf").to_vec());

        let icon_data = load_font(&[
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMono Nerd Font Regular.ttf",
            "/usr/share/fonts/TTF/FiraCodeNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/SymbolsNerdFont-Regular.ttf",
        ]);
        let has_nerd_font = icon_data.is_some();
        let text_font = Font::from_bytes(text_data.as_slice(), FontSettings::default()).expect("text font");
        let fonts = if let Some(b) = icon_data {
            match Font::from_bytes(b.as_slice(), FontSettings::default()) {
                Ok(f) => vec![text_font, f],
                Err(_) => vec![text_font],
            }
        } else { vec![text_font] };

        Self { fonts, layout: Layout::new(CoordinateSystem::PositiveYDown), has_nerd_font,
               glyph_cache: HashMap::with_capacity(512),
               layout_cache: HashMap::with_capacity(256),
               measure_cache: HashMap::with_capacity(256) }
    }

    pub fn draw(&mut self, pixmap: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: Color) -> f32 {
        if text.is_empty() || size < 1.0 { return 0.0; }
        let r = (color.red()   * 255.0) as u8;
        let g = (color.green() * 255.0) as u8;
        let b = (color.blue()  * 255.0) as u8;
        let a = (color.alpha() * 255.0) as u8;
        if a == 0 { return 0.0; }

        let pw = pixmap.width()  as i32;
        let ph = pixmap.height() as i32;
        let sk = (size * 2.0).round() as u16;
        let lk = MeasureKey(text.into(), sk);

        if !self.layout_cache.contains_key(&lk) {
            let mut entry = LayoutEntry { glyphs: Vec::new(), advance: 0.0 };
            let fonts_ref: Vec<&Font> = self.fonts.iter().collect();
            let has_nf = self.has_nerd_font;
            for_each_run(text, has_nf, |run, fi| {
                let fi = fi.min(fonts_ref.len() - 1);
                self.layout.reset(&Default::default());
                self.layout.append(&fonts_ref, &TextStyle::new(run, size, fi));
                let base = entry.advance;
                for gl in self.layout.glyphs() {
                    let fa = gl.font_index.min(fonts_ref.len() - 1);
                    entry.glyphs.push((GlyphKey(fa as u8, gl.parent, sk), base + gl.x, gl.y));
                }
                let rw = self.layout.glyphs().iter().map(|g| g.x + g.width as f32).fold(0.0f32, f32::max);
                entry.advance += rw;
            });
            self.layout_cache.insert(lk.clone(), entry);
        }

        let (gl_list, advance) = {
            let e = &self.layout_cache[&lk];
            (e.glyphs.clone(), e.advance)
        };
        let pixels = pixmap.pixels_mut();
        for (gk, rel_x, rel_y) in &gl_list {
            let cached = self.glyph_cache.entry(gk.clone()).or_insert_with(|| {
                let fi = gk.0 as usize;
                let (m, bm) = self.fonts[fi].rasterize(gk.1, size);
                CachedGlyph { width: m.width, height: m.height, bitmap: bm }
            });
            if cached.width == 0 || cached.height == 0 { continue; }
            let gx = (x + rel_x).round() as i32;
            let gy = (y + rel_y).round() as i32;
            for row in 0..cached.height {
                for col in 0..cached.width {
                    let px = gx + col as i32; let py = gy + row as i32;
                    if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
                    let cov = cached.bitmap[row * cached.width + col];
                    if cov == 0 { continue; }
                    let idx   = (py * pw + px) as usize;
                    let dst   = &mut pixels[idx];
                    let src_a = (cov as u16 * a as u16) / 255;
                    let inv_a = 255u16.saturating_sub(src_a);
                    let dr = ((r as u16 * src_a + dst.red()   as u16 * inv_a) / 255) as u8;
                    let dg = ((g as u16 * src_a + dst.green() as u16 * inv_a) / 255) as u8;
                    let db = ((b as u16 * src_a + dst.blue()  as u16 * inv_a) / 255) as u8;
                    let da = src_a.saturating_add(dst.alpha() as u16 * inv_a / 255) as u8;
                    *dst = tiny_skia::PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
                }
            }
        }
        advance
    }

    pub fn measure(&mut self, text: &str, size: f32) -> f32 {
        if text.is_empty() || size < 1.0 { return 0.0; }
        let sk = (size * 2.0).round() as u16;
        let k  = MeasureKey(text.into(), sk);
        if let Some(&v) = self.measure_cache.get(&k) { return v; }
        let mut total = 0.0f32;
        for_each_run(text, self.has_nerd_font, |run, fi| {
            let fi = fi.min(self.fonts.len() - 1);
            self.layout.reset(&Default::default());
            self.layout.append(&self.fonts.iter().collect::<Vec<_>>(), &TextStyle::new(run, size, fi));
            total += self.layout.glyphs().iter().map(|g| g.x + g.width as f32).fold(0.0f32, f32::max);
        });
        self.measure_cache.insert(k, total);
        total
    }

    pub fn clear_dynamic(&mut self) { self.layout_cache.clear(); self.measure_cache.clear(); }
}

fn for_each_run(text: &str, has_nf: bool, mut f: impl FnMut(&str, usize)) {
    if !has_nf { f(text, 0); return; }
    let mut run_start = 0usize;
    let mut run_fi    = 0usize;
    let mut first     = true;
    for (pos, ch) in text.char_indices() {
        let fi = if is_icon(ch) { 1 } else { 0 };
        if first { run_fi = fi; first = false; }
        else if fi != run_fi { f(&text[run_start..pos], run_fi); run_start = pos; run_fi = fi; }
    }
    if !first { f(&text[run_start..], run_fi); } else { f(text, 0); }
}

fn is_icon(ch: char) -> bool {
    let c = ch as u32;
    matches!(c, 0xE000..=0xF8FF | 0x2580..=0x259F | 0x25A0..=0x25FF | 0x2600..=0x26FF | 0x2700..=0x27BF)
}

fn load_font(paths: &[&str]) -> Option<Vec<u8>> {
    for p in paths {
        let expanded = if p.starts_with("~/") {
            format!("{}{}", std::env::var("HOME").unwrap_or_default(), &p[1..])
        } else { p.to_string() };
        if let Ok(d) = std::fs::read(&expanded) { return Some(d); }
    }
    None
}
