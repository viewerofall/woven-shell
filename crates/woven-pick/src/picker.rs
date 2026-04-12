//! Picker state, grid layout, rendering, and input handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use image::imageops::FilterType;
use tiny_skia::{Color, Pixmap};
use xkeysym::key;

use crate::draw::*;
use crate::text::TextRenderer;

// ─── layout constants ─────────────────────────────────────────────────────────

const THUMB_W:   u32  = 220;
const THUMB_H:   u32  = 138;
const NAME_H:    f32  = 30.0;
const CELL_H:    f32  = THUMB_H as f32 + NAME_H;
const GAP:       f32  = 14.0;
const HEADER_H:  f32  = 72.0;
const SIDE_PAD:  f32  = 40.0;
const CORNER_R:  f32  = 8.0;

// ─── theme ────────────────────────────────────────────────────────────────────

const BG:           &str = "f50a0010";   // #0a0010 + alpha
const CARD_BG:      &str = "ff0f0020";   // card background
const CARD_BG_HOV:  &str = "ff1a0035";   // hovered card
const SEL_COLOR:    &str = "ffc792ea";   // #c792ea selection border
const SEL_FILL:     &str = "33c792ea";   // selection fill (20% alpha)
const ACCENT:       &str = "ff00e5c8";   // #00e5c8 accent
const TEXT_PRI:     &str = "ffe8e0f0";   // primary text
const TEXT_SEC:     &str = "ff8888aa";   // secondary / filename
const SEARCH_BG:    &str = "ff15002a";   // search bar fill
const SEARCH_BORD:  &str = "99c792ea";   // search bar border
const CURSOR_COL:   &str = "ff00e5c8";   // text cursor

// ─── Picker ───────────────────────────────────────────────────────────────────

pub struct Picker {
    dir:        String,
    all_images: Vec<PathBuf>,
    filtered:   Vec<usize>,               // indices into all_images
    thumbs:     HashMap<usize, Vec<u8>>,  // RGBA thumb at THUMB_W×THUMB_H

    pub selected: usize,                  // index into filtered
    hovered:      Option<usize>,

    search:        String,
    cursor_blink:  u32,                   // frame counter for blink

    scroll_y:        f32,
    target_scroll_y: f32,
    sel_ax:          f32,                 // animated selection X
    sel_ay:          f32,                 // animated selection Y (content-space)

    pub should_close: bool,
    pub apply_path:   Option<PathBuf>,
    pub dirty:        bool,

    text:   TextRenderer,
    start:  Instant,
}

impl Picker {
    pub fn new(dir: &str) -> anyhow::Result<Self> {
        let all_images = collect_images(dir)?;
        if all_images.is_empty() {
            anyhow::bail!("pick: no images in {dir}");
        }
        let filtered: Vec<usize> = (0..all_images.len()).collect();
        Ok(Self {
            dir: dir.to_string(),
            all_images,
            filtered,
            thumbs:         HashMap::new(),
            selected:       0,
            hovered:        None,
            search:         String::new(),
            cursor_blink:   0,
            scroll_y:       0.0,
            target_scroll_y: 0.0,
            sel_ax:         SIDE_PAD,
            sel_ay:         HEADER_H + GAP,
            should_close:   false,
            apply_path:     None,
            dirty:          true,
            text:           TextRenderer::new(),
            start:          Instant::now(),
        })
    }

    pub fn mark_dirty(&mut self) { self.dirty = true; }

    pub fn is_animating(&self) -> bool {
        (self.target_scroll_y - self.scroll_y).abs() > 0.3
        || (self.sel_ay - self.target_sel_ay()).abs() > 0.3
        || self.dirty
    }

    fn target_sel_ay(&self) -> f32 {
        // sel_ay target = content-space y of selected cell (same as cell_content_pos)
        // We recompute here without needing cols — approximate using stored sel_ay target
        // (the real target is updated in render; this is just for animation detection)
        self.sel_ay  // close enough — animation guard only needs rough check
    }

    // ── input ─────────────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, raw: u32, utf8: Option<&str>, screen_w: u32, screen_h: u32) {
        let cols = self.cols(screen_w);
        let rows = self.rows();

        match raw {
            key::Escape => { self.should_close = true; }
            key::Return | key::KP_Enter => { self.apply_selected(); }
            key::BackSpace => {
                self.search.pop();
                self.text.clear_dynamic();
                self.refilter();
            }
            key::Up => {
                if self.selected >= cols { self.selected -= cols; }
                self.on_sel_changed(screen_w, screen_h);
            }
            key::Down => {
                if self.selected + cols < self.filtered.len() { self.selected += cols; }
                else { self.selected = self.filtered.len().saturating_sub(1); }
                self.on_sel_changed(screen_w, screen_h);
            }
            key::Left => {
                if self.selected > 0 { self.selected -= 1; }
                self.on_sel_changed(screen_w, screen_h);
            }
            key::Right => {
                if self.selected + 1 < self.filtered.len() { self.selected += 1; }
                self.on_sel_changed(screen_w, screen_h);
            }
            key::Page_Up => {
                let page_rows = (((screen_h as f32 - HEADER_H) / (CELL_H + GAP)) as usize).max(1);
                self.selected = self.selected.saturating_sub(page_rows * cols);
                self.on_sel_changed(screen_w, screen_h);
            }
            key::Page_Down => {
                let page_rows = (((screen_h as f32 - HEADER_H) / (CELL_H + GAP)) as usize).max(1);
                let new = (self.selected + page_rows * cols).min(self.filtered.len().saturating_sub(1));
                self.selected = new;
                self.on_sel_changed(screen_w, screen_h);
            }
            _ => {
                if let Some(s) = utf8 {
                    let s = s.chars().filter(|c| !c.is_control()).collect::<String>();
                    if !s.is_empty() {
                        self.search.push_str(&s);
                        self.text.clear_dynamic();
                        self.refilter();
                    }
                }
            }
        }
        let _ = rows; // suppress unused
    }

    pub fn handle_pointer_move(&mut self, mx: f64, my: f64, screen_w: u32) {
        let cols = self.cols(screen_w);
        let content_y = my as f32 - HEADER_H + self.scroll_y;
        if content_y < GAP || my < HEADER_H as f64 { self.hovered = None; return; }
        let col = ((mx as f32 - SIDE_PAD) / (THUMB_W as f32 + GAP)) as usize;
        let row = ((content_y - GAP) / (CELL_H + GAP)) as usize;
        let idx = row * cols + col;
        if idx < self.filtered.len() && col < cols { self.hovered = Some(idx); }
        else { self.hovered = None; }
    }

    pub fn handle_click(&mut self, mx: f64, my: f64, screen_w: u32, screen_h: u32) {
        let cols = self.cols(screen_w);
        let content_y = my as f32 - HEADER_H + self.scroll_y;
        if content_y < GAP || my < HEADER_H as f64 { return; }
        let col = ((mx as f32 - SIDE_PAD) / (THUMB_W as f32 + GAP)) as usize;
        let row = ((content_y - GAP) / (CELL_H + GAP)) as usize;
        let idx = row * cols + col;
        if idx < self.filtered.len() && col < cols {
            if self.selected == idx {
                self.apply_selected();
            } else {
                self.selected = idx;
                self.on_sel_changed(screen_w, screen_h);
            }
        }
    }

    pub fn handle_scroll(&mut self, dy: f64, screen_w: u32, screen_h: u32) {
        let step = CELL_H + GAP;
        self.target_scroll_y += dy as f32 * step * 0.4;
        self.clamp_scroll(screen_w, screen_h);
    }

    // ── render ────────────────────────────────────────────────────────────────

    pub fn render(&mut self, width: u32, height: u32) -> Vec<u8> {
        self.dirty = false;
        self.cursor_blink = self.cursor_blink.wrapping_add(1);

        // animate scroll
        let ds = (self.target_scroll_y - self.scroll_y) * 0.18;
        if ds.abs() > 0.3 { self.scroll_y += ds; self.dirty = true; }
        else { self.scroll_y = self.target_scroll_y; }

        let cols = self.cols(width);
        let (sel_tx, sel_ty) = self.cell_content_pos(self.selected, cols);

        // animate selection cursor
        let dx = sel_tx - self.sel_ax; let dy = sel_ty - self.sel_ay;
        if dx.abs() > 0.3 { self.sel_ax += dx * 0.2; self.dirty = true; } else { self.sel_ax = sel_tx; }
        if dy.abs() > 0.3 { self.sel_ay += dy * 0.2; self.dirty = true; } else { self.sel_ay = sel_ty; }

        // cursor blink — mark dirty every 30 frames
        if self.cursor_blink % 30 == 0 && !self.search.is_empty() { self.dirty = true; }

        let mut pixmap = match Pixmap::new(width, height) {
            Some(p) => p,
            None    => return vec![0u8; (width * height * 4) as usize],
        };

        // background
        pixmap.fill(hex_color(BG));

        // ── header ───────────────────────────────────────────────────────────
        fill_rect(&mut pixmap, 0.0, 0.0, width as f32, HEADER_H, hex_color("ff0c001e"));

        // title
        let title = format!("  Wallpapers  ({})", self.filtered.len());
        self.text.draw(&mut pixmap, &title, 16.0, 18.0, 18.0, hex_color(TEXT_SEC));

        // search bar
        let sb_w = 380.0f32;
        let sb_h = 36.0f32;
        let sb_x = (width as f32 - sb_w) / 2.0;
        let sb_y = (HEADER_H - sb_h) / 2.0;
        fill_rounded_rect(&mut pixmap, sb_x, sb_y, sb_w, sb_h, 6.0, hex_color(SEARCH_BG));
        stroke_rounded_rect(&mut pixmap, sb_x, sb_y, sb_w, sb_h, 6.0, hex_color(SEARCH_BORD), 1.5);

        let placeholder_active = self.search.is_empty();
        let display_text = if placeholder_active { "Search..." } else { &self.search };
        let txt_color    = if placeholder_active { hex_color(TEXT_SEC) } else { hex_color(TEXT_PRI) };
        let icon_w = self.text.draw(&mut pixmap, "", sb_x + 10.0, sb_y + 8.0, 16.0, hex_color(ACCENT));
        self.text.draw(&mut pixmap, display_text, sb_x + 14.0 + icon_w, sb_y + 9.0, 15.0, txt_color);

        // text cursor blink in search bar (every 30 frames)
        if !placeholder_active && (self.cursor_blink / 30) % 2 == 0 {
            let cursor_x = sb_x + 14.0 + icon_w + self.text.measure(&self.search, 15.0) + 1.0;
            fill_rect(&mut pixmap, cursor_x, sb_y + 8.0, 2.0, sb_h - 16.0, hex_color(CURSOR_COL));
        }

        // hint right side
        let hint = "↵ apply  Esc close";
        let hw = self.text.measure(hint, 12.0);
        self.text.draw(&mut pixmap, hint, width as f32 - hw - 16.0, 27.0, 12.0, hex_color(TEXT_SEC));

        // separator line
        fill_rect(&mut pixmap, 0.0, HEADER_H - 1.0, width as f32, 1.0, hex_color("33c792ea"));

        // ── grid ─────────────────────────────────────────────────────────────

        let grid_top  = HEADER_H;

        // pre-load thumbnails: visible rows + 1 row above + 2 rows below (look-ahead)
        let lookahead_top    = grid_top - (CELL_H + GAP);
        let lookahead_bottom = height as f32 + 2.0 * (CELL_H + GAP);
        let to_load: Vec<usize> = self.filtered.iter().enumerate()
            .filter_map(|(fi, &img_idx)| {
                let (_, cy) = self.cell_content_pos(fi, cols);
                let sy = cy - self.scroll_y;
                if sy + CELL_H >= lookahead_top && sy <= lookahead_bottom
                    && !self.thumbs.contains_key(&img_idx)
                { Some(img_idx) } else { None }
            })
            .collect();
        // load at most 3 per frame to avoid stalling
        for img_idx in to_load.into_iter().take(3) {
            self.ensure_thumb(img_idx);
            self.dirty = true; // re-render to show newly loaded thumb
        }

        for (fi, &img_idx) in self.filtered.iter().enumerate() {
            let (cx, cy) = self.cell_content_pos(fi, cols);
            let screen_y = cy - self.scroll_y;

            // cull invisible rows
            if screen_y + CELL_H < grid_top { continue; }
            if screen_y > height as f32 { continue; }

            let is_sel  = fi == self.selected;
            let is_hov  = self.hovered == Some(fi);

            // card bg
            let card_color = if is_hov { hex_color(CARD_BG_HOV) } else { hex_color(CARD_BG) };
            let clip_top    = screen_y.max(grid_top);
            let clip_bottom = (screen_y + CELL_H).min(height as f32);
            if clip_bottom > clip_top {
                let visible_h = clip_bottom - clip_top;
                let offset_y  = clip_top - screen_y;
                fill_rounded_rect(&mut pixmap,
                    cx, clip_top, THUMB_W as f32, visible_h.min(CELL_H - offset_y),
                    CORNER_R, card_color);
            }

            // thumbnail
            if let Some(rgba) = self.thumbs.get(&img_idx) {
                let thumb_screen_y = screen_y;
                let thumb_clip_top    = thumb_screen_y.max(grid_top);
                let thumb_clip_bottom = (thumb_screen_y + THUMB_H as f32).min(height as f32);
                if thumb_clip_bottom > thumb_clip_top {
                    let clip_rows = (thumb_clip_bottom - thumb_clip_top) as u32;
                    let dst_y     = thumb_clip_top as i32;
                    let src_y_off = (thumb_clip_top - thumb_screen_y) as u32;
                    // blit with row offset
                    blit_thumb_offset(&mut pixmap,
                        cx as i32, dst_y, THUMB_W, THUMB_H, rgba, src_y_off, clip_rows, CORNER_R);
                }
            } else {
                // loading placeholder (thumb scheduled above)
                let ph_screen_y = screen_y.max(grid_top);
                if ph_screen_y < height as f32 {
                    fill_rounded_rect(&mut pixmap, cx, ph_screen_y, THUMB_W as f32,
                        (screen_y + THUMB_H as f32).min(height as f32) - ph_screen_y,
                        CORNER_R, hex_color("ff0a0022"));
                    self.text.draw(&mut pixmap, "...", cx + THUMB_W as f32 / 2.0 - 10.0,
                        ph_screen_y + THUMB_H as f32 / 2.0 - 8.0, 14.0, hex_color(TEXT_SEC));
                }
            }

            // filename label (below thumb)
            let name_y = screen_y + THUMB_H as f32;
            if name_y < height as f32 && name_y + NAME_H > grid_top {
                let path  = &self.all_images[img_idx];
                let fname = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                let fname = truncate_str(fname, THUMB_W as usize / 8);
                let tw    = self.text.measure(fname, 12.0);
                let tx    = cx + (THUMB_W as f32 - tw) / 2.0;
                let ty    = name_y + (NAME_H - 14.0) / 2.0;
                let tc    = if is_sel { hex_color(TEXT_PRI) } else { hex_color(TEXT_SEC) };
                if ty < height as f32 { self.text.draw(&mut pixmap, fname, tx, ty, 12.0, tc); }
            }
        }

        // ── animated selection box ────────────────────────────────────────────
        let bx = self.sel_ax;
        let by = self.sel_ay - self.scroll_y;
        if by + CELL_H > grid_top && by < height as f32 {
            let clip_top = by.max(grid_top);
            let clip_h   = (by + CELL_H).min(height as f32) - clip_top;

            // selection fill
            fill_rounded_rect(&mut pixmap, bx, clip_top, THUMB_W as f32, clip_h, CORNER_R, hex_color(SEL_FILL));

            // selection border — draw all four sides clipped
            stroke_rounded_rect(&mut pixmap, bx, clip_top,
                THUMB_W as f32, (by + CELL_H).min(height as f32) - clip_top,
                CORNER_R, hex_color(SEL_COLOR), 2.5);
        }

        // ── fade at top/bottom of grid ────────────────────────────────────────
        gradient_fade(&mut pixmap, 0.0, HEADER_H, width as f32, 20.0, true,  hex_color("0c001e"));
        gradient_fade(&mut pixmap, 0.0, height as f32 - 24.0, width as f32, 24.0, false, hex_color("0a0010"));

        pixmap_to_bgra(pixmap)
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn cols(&self, screen_w: u32) -> usize {
        let usable = screen_w as f32 - SIDE_PAD * 2.0;
        ((usable + GAP) / (THUMB_W as f32 + GAP)).max(1.0) as usize
    }

    fn rows(&self) -> usize {
        (self.filtered.len() + self.cols(1920).saturating_sub(1)) / self.cols(1920).max(1)
    }

    /// Content-space (x, y) of a cell, ignoring scroll.
    fn cell_content_pos(&self, fi: usize, cols: usize) -> (f32, f32) {
        let col = fi % cols;
        let row = fi / cols;
        let x   = SIDE_PAD + col as f32 * (THUMB_W as f32 + GAP);
        let y   = HEADER_H + GAP + row as f32 * (CELL_H + GAP);
        (x, y)
    }

    fn content_height(&self, screen_w: u32) -> f32 {
        let cols = self.cols(screen_w);
        let rows = (self.filtered.len() + cols - 1) / cols;
        HEADER_H + GAP + rows as f32 * (CELL_H + GAP) + GAP
    }

    fn clamp_scroll(&mut self, screen_w: u32, screen_h: u32) {
        let max = (self.content_height(screen_w) - screen_h as f32).max(0.0);
        self.target_scroll_y = self.target_scroll_y.clamp(0.0, max);
    }

    fn on_sel_changed(&mut self, screen_w: u32, screen_h: u32) {
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        let cols = self.cols(screen_w);
        let (_, sel_cy) = self.cell_content_pos(self.selected, cols);

        // ensure visible
        let vis_top = self.target_scroll_y + HEADER_H;
        let vis_bot = self.target_scroll_y + screen_h as f32;
        if sel_cy < vis_top + GAP {
            self.target_scroll_y = (sel_cy - HEADER_H - GAP).max(0.0);
        } else if sel_cy + CELL_H > vis_bot - GAP {
            self.target_scroll_y = sel_cy + CELL_H - screen_h as f32 + GAP;
        }
        self.clamp_scroll(screen_w, screen_h);
    }

    fn apply_selected(&mut self) {
        if let Some(&img_idx) = self.filtered.get(self.selected) {
            self.apply_path = Some(self.all_images[img_idx].clone());
            self.should_close = true;
        }
    }

    fn refilter(&mut self) {
        let q = self.search.to_lowercase();
        self.filtered = if q.is_empty() {
            (0..self.all_images.len()).collect()
        } else {
            (0..self.all_images.len())
                .filter(|&i| {
                    self.all_images[i]
                        .file_name().and_then(|n| n.to_str())
                        .map(|n| n.to_lowercase().contains(&q))
                        .unwrap_or(false)
                })
                .collect()
        };
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    fn ensure_thumb(&mut self, img_idx: usize) {
        if self.thumbs.contains_key(&img_idx) { return; }
        let path = &self.all_images[img_idx];
        match image::open(path) {
            Ok(img) => {
                let scale_x = THUMB_W as f32 / img.width()  as f32;
                let scale_y = THUMB_H as f32 / img.height() as f32;
                let scale   = scale_x.max(scale_y);
                let nw      = (img.width()  as f32 * scale).ceil() as u32;
                let nh      = (img.height() as f32 * scale).ceil() as u32;
                let scaled  = img.resize(nw, nh, FilterType::Triangle);
                let cx      = (nw.saturating_sub(THUMB_W)) / 2;
                let cy      = (nh.saturating_sub(THUMB_H)) / 2;
                let cropped = scaled.crop_imm(cx, cy, THUMB_W, THUMB_H);
                let rgba    = cropped.to_rgba8().into_raw();
                self.thumbs.insert(img_idx, rgba);
            }
            Err(e) => {
                tracing::warn!("pick: thumb failed for {}: {e}", path.display());
                self.thumbs.insert(img_idx, vec![20u8; (THUMB_W * THUMB_H * 4) as usize]);
            }
        }
    }
}

// ─── pure helpers ─────────────────────────────────────────────────────────────

fn collect_images(dir: &str) -> anyhow::Result<Vec<PathBuf>> {
    let expanded = if let Some(rest) = dir.strip_prefix("~/") {
        format!("{}/{rest}", std::env::var("HOME").unwrap_or_default())
    } else { dir.to_string() };

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&expanded)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && matches!(
            p.extension().and_then(|x| x.to_str()).map(|x| x.to_lowercase()).as_deref(),
            Some("png" | "jpg" | "jpeg" | "webp")
        ))
        .collect();
    paths.sort();
    Ok(paths)
}

fn truncate_str(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars { return s; }
    let mut end = max_chars;
    while !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

fn pixmap_to_bgra(pixmap: Pixmap) -> Vec<u8> {
    let data = pixmap.data();
    let mut out = Vec::with_capacity(data.len());
    for px in data.chunks_exact(4) {
        out.push(px[2]);
        out.push(px[1]);
        out.push(px[0]);
        out.push(px[3]);
    }
    out
}

/// Blit thumbnail RGBA starting from src row `src_y_off`, for `clip_rows` rows.
fn blit_thumb_offset(
    pixmap: &mut Pixmap,
    x: i32, y: i32,
    w: u32, h: u32,
    rgba: &[u8],
    src_y_off: u32,
    clip_rows: u32,
    _corner_r: f32,
) {
    let pw = pixmap.width()  as i32;
    let ph = pixmap.height() as i32;
    let pixels = pixmap.pixels_mut();

    for dy in 0..clip_rows as i32 {
        let src_row = src_y_off as i32 + dy;
        if src_row >= h as i32 { break; }
        for dx in 0..w as i32 {
            let px = x + dx; let py = y + dy;
            if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
            let si = (src_row as usize * w as usize + dx as usize) * 4;
            if si + 3 >= rgba.len() { continue; }
            let (sr, sg, sb, sa) = (rgba[si], rgba[si+1], rgba[si+2], rgba[si+3]);
            if sa == 0 { continue; }
            let idx   = (py * pw + px) as usize;
            let dst   = &mut pixels[idx];
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

/// Simple vertical gradient fade (for top/bottom edge softening).
fn gradient_fade(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, top_opaque: bool, color: Color) {
    let steps = h as u32;
    if steps == 0 { return; }
    for i in 0..steps {
        let t    = i as f32 / steps as f32;
        let a    = if top_opaque { 1.0 - t } else { t };
        let mut c = color;
        c.set_alpha(a * 0.92);
        fill_rect(pixmap, x, y + i as f32, w, 1.0, c);
    }
}
