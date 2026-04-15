//! Launcher renderer — floating centered panel with search, results, calculator.

use crate::calc;
use crate::config::LauncherSettings;
use crate::desktop::DesktopEntry;
use crate::draw::*;
use crate::icons::IconCache;
use crate::search::{self, SearchResult};
use crate::text::TextRenderer;
use std::time::Instant;
use tiny_skia::{Color, Pixmap};

// ─── Layout constants ────────────────────────────────────────────────────────

const PANEL_R: f32 = 16.0;       // panel corner radius
const PANEL_PAD: f32 = 20.0;     // inner padding
const SEARCH_H: f32 = 48.0;      // search bar height
const SEARCH_R: f32 = 12.0;      // search bar corner radius
const DIVIDER_H: f32 = 24.0;     // gap between search and results
const RESULT_H: f32 = 52.0;      // each result row
const RESULT_R: f32 = 10.0;      // result row corner radius
const RESULT_GAP: f32 = 4.0;     // gap between result rows
const FOOTER_H: f32 = 36.0;      // footer hints
const BACKDROP_ALPHA: u8 = 0x88;  // dim overlay behind panel

pub struct LaunchRenderer {
    pub query: String,
    pub selected: usize,
    pub entries: Vec<DesktopEntry>,
    pub results: Vec<SearchResult>,
    pub should_close: bool,
    pub launch_exec: Option<String>,

    // scroll state — index of first visible result
    pub scroll_offset: usize,
    pub hovered: Option<usize>, // index into results

    pub text: TextRenderer,
    pub icons: IconCache,
    cursor_blink: Instant,
    pub dirty: bool,
    cfg: LauncherSettings,

    // cached panel geometry for hit testing
    panel_x: f32,
    panel_y: f32,
    panel_w: f32,
    results_y: f32, // y where results start on screen
    screen_w: u32,
    screen_h: u32,
}

impl LaunchRenderer {
    pub fn new(cfg: LauncherSettings, entries: Vec<DesktopEntry>) -> Self {
        let results = search::fuzzy_search(&entries, "");
        Self {
            query: String::new(),
            selected: 0,
            entries,
            results,
            should_close: false,
            launch_exec: None,
            scroll_offset: 0,
            hovered: None,
            text: TextRenderer::new(),
            icons: IconCache::new(),
            cursor_blink: Instant::now(),
            dirty: true,
            cfg,
            panel_x: 0.0,
            panel_y: 0.0,
            panel_w: 0.0,
            results_y: 0.0,
            screen_w: 0,
            screen_h: 0,
        }
    }

    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.text.clear_dynamic_cache();
        self.refilter();
        self.dirty = true;
        self.cursor_blink = Instant::now();
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
        self.text.clear_dynamic_cache();
        self.refilter();
        self.dirty = true;
        self.cursor_blink = Instant::now();
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.text.clear_dynamic_cache();
        self.refilter();
        self.dirty = true;
    }

    pub fn select_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
        self.dirty = true;
    }

    pub fn select_down(&mut self) {
        let max = self.results.len().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
            self.ensure_visible();
        }
        self.dirty = true;
    }

    pub fn page_up(&mut self) {
        let page = self.cfg.max_results;
        self.selected = self.selected.saturating_sub(page);
        self.ensure_visible();
        self.dirty = true;
    }

    pub fn page_down(&mut self) {
        let page = self.cfg.max_results;
        let max = self.results.len().saturating_sub(1);
        self.selected = (self.selected + page).min(max);
        self.ensure_visible();
        self.dirty = true;
    }

    pub fn scroll(&mut self, delta: f64) {
        if self.is_calc_mode() || self.is_cmd_mode() { return; }
        let max_offset = self.results.len().saturating_sub(self.cfg.max_results);
        if delta > 0.0 {
            self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
        } else if delta < 0.0 {
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
        // clamp selected to visible range
        if self.selected < self.scroll_offset {
            self.selected = self.scroll_offset;
        } else if self.selected >= self.scroll_offset + self.cfg.max_results {
            self.selected = self.scroll_offset + self.cfg.max_results - 1;
        }
        self.dirty = true;
    }

    pub fn handle_mouse_move(&mut self, mx: f64, my: f64) {
        if self.is_calc_mode() || self.is_cmd_mode() { return; }
        let hit = self.hit_test_result(mx, my);
        if hit != self.hovered {
            self.hovered = hit;
            self.dirty = true;
        }
    }

    pub fn handle_click(&mut self, mx: f64, my: f64) {
        // click outside panel = close
        let mx = mx as f32;
        let my = my as f32;
        if mx < self.panel_x || mx > self.panel_x + self.panel_w
            || my < self.panel_y || my > self.panel_y + self.panel_height()
        {
            self.should_close = true;
            return;
        }

        if self.is_calc_mode() || self.is_cmd_mode() { return; }

        if let Some(idx) = self.hit_test_result(mx as f64, my as f64) {
            self.selected = idx;
            self.confirm();
        }
    }

    fn hit_test_result(&self, mx: f64, my: f64) -> Option<usize> {
        let mx = mx as f32;
        let my = my as f32;
        let inner_x = self.panel_x + PANEL_PAD;
        let inner_w = self.panel_w - PANEL_PAD * 2.0;

        if mx < inner_x || mx > inner_x + inner_w { return None; }

        let vis = self.visible_count();
        for i in 0..vis {
            let abs_idx = self.scroll_offset + i;
            let ry = self.results_y + i as f32 * (RESULT_H + RESULT_GAP);
            if my >= ry && my < ry + RESULT_H {
                return Some(abs_idx);
            }
        }
        None
    }

    pub fn confirm(&mut self) {
        // calculator mode — nothing to launch
        if self.is_calc_mode() { return; }

        // command runner mode
        if self.is_cmd_mode() {
            let cmd = self.query[1..].trim().to_string();
            if !cmd.is_empty() {
                self.launch_exec = Some(cmd);
                self.should_close = true;
            }
            return;
        }

        // launch selected app
        if let Some(result) = self.results.get(self.selected) {
            let entry = &self.entries[result.index];
            let exec = if entry.terminal {
                format!("kitty -e {}", entry.exec)
            } else {
                entry.exec.clone()
            };
            self.launch_exec = Some(exec);
            self.should_close = true;
        }
    }

    pub fn is_animating(&self) -> bool {
        self.dirty || self.cursor_blink.elapsed().as_millis() % 1000 < 16
    }

    fn is_calc_mode(&self) -> bool {
        self.cfg.calculator && self.query.starts_with('=')
    }

    fn is_cmd_mode(&self) -> bool {
        self.cfg.command_runner && self.query.starts_with('!')
    }

    fn visible_count(&self) -> usize {
        if self.is_calc_mode() || self.is_cmd_mode() {
            1
        } else {
            let remaining = self.results.len().saturating_sub(self.scroll_offset);
            remaining.min(self.cfg.max_results)
        }
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.cfg.max_results {
            self.scroll_offset = self.selected + 1 - self.cfg.max_results;
        }
    }

    fn refilter(&mut self) {
        if self.is_calc_mode() || self.is_cmd_mode() {
            self.results.clear();
            self.selected = 0;
            self.scroll_offset = 0;
            return;
        }
        self.results = search::fuzzy_search(&self.entries, &self.query);
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn panel_height(&self) -> f32 {
        let n = self.visible_count();
        PANEL_PAD * 2.0
            + SEARCH_H
            + if n > 0 { DIVIDER_H + n as f32 * RESULT_H + (n.saturating_sub(1)) as f32 * RESULT_GAP } else { 0.0 }
            + FOOTER_H
    }

    /// Render the full overlay. Returns BGRA pixels.
    pub fn render(&mut self, w: u32, h: u32) -> Vec<u8> {
        self.dirty = false;
        self.screen_w = w;
        self.screen_h = h;
        let mut pixmap = Pixmap::new(w, h).expect("pixmap");

        // 1. Dim backdrop
        fill_rect(&mut pixmap, 0.0, 0.0, w as f32, h as f32,
            Color::from_rgba8(0x0a, 0x00, 0x10, BACKDROP_ALPHA));

        let panel_w = self.cfg.width as f32;
        let panel_h = self.panel_height();
        let px = (w as f32 - panel_w) / 2.0;
        let py = (h as f32 - panel_h) / 2.0 - 40.0; // slightly above center

        self.panel_x = px;
        self.panel_y = py;
        self.panel_w = panel_w;

        // 2. Panel background
        fill_rounded_rect(&mut pixmap, px, py, panel_w, panel_h, PANEL_R,
            hex_color(&self.cfg.panel_background));

        // 3. Panel border (subtle)
        stroke_rounded_rect(&mut pixmap, px, py, panel_w, panel_h, PANEL_R,
            with_alpha(hex_color(&self.cfg.border_color), 0.25), 1.5);

        let inner_x = px + PANEL_PAD;
        let inner_w = panel_w - PANEL_PAD * 2.0;
        let mut cy = py + PANEL_PAD;

        // 4. Search bar
        let search_bg = with_alpha(hex_color(&self.cfg.background), 0.6);
        fill_rounded_rect(&mut pixmap, inner_x, cy, inner_w, SEARCH_H, SEARCH_R, search_bg);
        stroke_rounded_rect(&mut pixmap, inner_x, cy, inner_w, SEARCH_H, SEARCH_R,
            with_alpha(hex_color(&self.cfg.accent_color), 0.4), 1.0);

        // search icon
        let icon_color = hex_color(&self.cfg.accent_color);
        let icon_w = self.text.draw(&mut pixmap, "", inner_x + 16.0, cy + 14.0, 18.0, icon_color);

        // search text / placeholder
        let text_x = inner_x + 16.0 + icon_w + 10.0;
        let text_y = cy + (SEARCH_H - 18.0) / 2.0;
        if self.query.is_empty() {
            let ph = if self.cfg.calculator && self.cfg.command_runner {
                "Search apps, = calc, ! command..."
            } else if self.cfg.calculator {
                "Search apps, = calc..."
            } else {
                "Search apps..."
            };
            self.text.draw(&mut pixmap, ph, text_x, text_y, 16.0,
                with_alpha(hex_color(&self.cfg.text_dim), 0.6));
        } else {
            self.text.draw(&mut pixmap, &self.query, text_x, text_y, 16.0,
                hex_color(&self.cfg.text_color));
        }

        // cursor blink
        let blink_ms = self.cursor_blink.elapsed().as_millis() % 1000;
        if blink_ms < 500 {
            let cursor_x = if self.query.is_empty() {
                text_x
            } else {
                text_x + self.text.measure(&self.query, 16.0) + 2.0
            };
            fill_rect(&mut pixmap, cursor_x, cy + 12.0, 2.0, SEARCH_H - 24.0,
                with_alpha(hex_color(&self.cfg.accent_color), 0.9));
        }

        cy += SEARCH_H;

        // 5. Results / calc / cmd
        let n = self.visible_count();
        if n > 0 {
            cy += DIVIDER_H;

            // thin separator line
            fill_rect(&mut pixmap, inner_x + 20.0, cy - DIVIDER_H / 2.0,
                inner_w - 40.0, 1.0,
                with_alpha(hex_color(&self.cfg.border_color), 0.12));

            self.results_y = cy;

            if self.is_calc_mode() {
                self.render_calc_result(&mut pixmap, inner_x, cy, inner_w);
            } else if self.is_cmd_mode() {
                self.render_cmd_hint(&mut pixmap, inner_x, cy, inner_w);
            } else {
                self.render_results(&mut pixmap, inner_x, cy, inner_w);
            }
        }

        // 6. Footer hints
        let footer_y = py + panel_h - FOOTER_H;
        let hint_color = with_alpha(hex_color(&self.cfg.text_dim), 0.5);

        let hints = if self.is_calc_mode() {
            "= calculator mode    Esc close".to_string()
        } else if self.is_cmd_mode() {
            "↵ run command    Esc close".to_string()
        } else if self.results.len() > self.cfg.max_results {
            format!("↑↓ navigate    ↵ launch    Esc close    {} of {}",
                self.selected + 1, self.results.len())
        } else {
            "↑↓ navigate    ↵ launch    Esc close".to_string()
        };
        let hw = self.text.measure(&hints, 12.0);
        self.text.draw(&mut pixmap, &hints,
            px + (panel_w - hw) / 2.0, footer_y + 10.0, 12.0, hint_color);

        pixmap_to_bgra(pixmap)
    }

    fn render_results(&mut self, pixmap: &mut Pixmap, x: f32, mut y: f32, w: f32) {
        let vis = self.visible_count();

        // scroll indicator top
        if self.scroll_offset > 0 {
            let arrow = "▲";
            let aw = self.text.measure(arrow, 10.0);
            self.text.draw(pixmap, arrow, x + (w - aw) / 2.0, y - 14.0, 10.0,
                with_alpha(hex_color(&self.cfg.text_dim), 0.4));
        }

        for i in 0..vis {
            let abs_idx = self.scroll_offset + i;
            let result = &self.results[abs_idx];
            let entry = &self.entries[result.index];
            let is_sel = abs_idx == self.selected;
            let is_hov = self.hovered == Some(abs_idx);

            // selection / hover highlight
            if is_sel {
                fill_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
                    hex_color(&self.cfg.selection_color));
                stroke_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
                    with_alpha(hex_color(&self.cfg.accent_color), 0.3), 1.0);
            } else if is_hov {
                fill_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
                    with_alpha(hex_color(&self.cfg.selection_color), 0.5));
            }

            // app icon or fallback initial circle
            let icon_name = entry.icon.clone();
            let icon_sz = self.icons.icon_size();
            let icon_x = (x + 28.0 - icon_sz as f32 / 2.0) as i32;
            let icon_y = (y + (RESULT_H - icon_sz as f32) / 2.0) as i32;

            if let Some(rgba) = self.icons.get(&icon_name) {
                blit_icon_rgba(pixmap, icon_x, icon_y, icon_sz, rgba);
            } else {
                // fallback: colored circle with initial letter
                let circle_x = x + 28.0;
                let circle_y = y + RESULT_H / 2.0;
                let circle_color = if is_sel {
                    with_alpha(hex_color(&self.cfg.accent_color), 0.8)
                } else {
                    with_alpha(hex_color(&self.cfg.accent_color), 0.3)
                };
                fill_circle(pixmap, circle_x, circle_y, 14.0, circle_color);

                let initial = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                let iw = self.text.measure(&initial, 14.0);
                self.text.draw(pixmap, &initial,
                    circle_x - iw / 2.0, circle_y - 8.0, 14.0,
                    hex_color(&self.cfg.text_color));
            }

            // app name
            let name_x = x + 56.0;
            let name_y = y + if entry.comment.is_empty() {
                (RESULT_H - 16.0) / 2.0
            } else {
                12.0
            };
            let name_color = if is_sel {
                hex_color(&self.cfg.text_color)
            } else {
                with_alpha(hex_color(&self.cfg.text_color), 0.8)
            };
            self.text.draw(pixmap, &entry.name, name_x, name_y, 15.0, name_color);

            // description (if present)
            if !entry.comment.is_empty() {
                let desc = truncate(&entry.comment, 60);
                self.text.draw(pixmap, &desc, name_x, name_y + 20.0, 12.0,
                    with_alpha(hex_color(&self.cfg.text_dim), 0.7));
            }

            // category label on right
            let cat = entry.category_label();
            let cw = self.text.measure(cat, 11.0);
            let cx = x + w - cw - 16.0;
            let cat_y = y + (RESULT_H - 11.0) / 2.0;
            self.text.draw(pixmap, cat, cx, cat_y, 11.0,
                with_alpha(hex_color(&self.cfg.text_dim), 0.5));

            y += RESULT_H + RESULT_GAP;
        }

        // scroll indicator bottom
        if self.scroll_offset + vis < self.results.len() {
            let arrow = "▼";
            let aw = self.text.measure(arrow, 10.0);
            self.text.draw(pixmap, arrow, x + (w - aw) / 2.0, y + 2.0, 10.0,
                with_alpha(hex_color(&self.cfg.text_dim), 0.4));
        }
    }

    fn render_calc_result(&mut self, pixmap: &mut Pixmap, x: f32, y: f32, w: f32) {
        let expr = &self.query[1..]; // strip '='

        // result row background
        fill_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
            hex_color(&self.cfg.selection_color));

        match calc::evaluate(expr) {
            Some(val) => {
                // format nicely
                let display = if val == val.floor() && val.abs() < 1e15 {
                    format!("= {}", val as i64)
                } else {
                    format!("= {:.6}", val).trim_end_matches('0').trim_end_matches('.').to_string()
                };

                let label_x = x + 20.0;
                let label_y = y + (RESULT_H - 20.0) / 2.0;
                self.text.draw(pixmap, &display, label_x, label_y, 20.0,
                    hex_color(&self.cfg.accent_color));
            }
            None => {
                if !expr.trim().is_empty() {
                    let label_x = x + 20.0;
                    let label_y = y + (RESULT_H - 14.0) / 2.0;
                    self.text.draw(pixmap, "invalid expression", label_x, label_y, 14.0,
                        with_alpha(hex_color(&self.cfg.text_dim), 0.5));
                }
            }
        }
    }

    fn render_cmd_hint(&mut self, pixmap: &mut Pixmap, x: f32, y: f32, w: f32) {
        let cmd = self.query[1..].trim().to_string();

        fill_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
            hex_color(&self.cfg.selection_color));

        let label_x = x + 20.0;
        let label_y = y + (RESULT_H - 14.0) / 2.0;

        if cmd.is_empty() {
            self.text.draw(pixmap, "type a command to run...", label_x, label_y, 14.0,
                with_alpha(hex_color(&self.cfg.text_dim), 0.5));
        } else {
            // show $ command
            let prompt_w = self.text.draw(pixmap, "$ ", label_x, label_y, 14.0,
                hex_color(&self.cfg.accent_color));
            self.text.draw(pixmap, &cmd, label_x + prompt_w, label_y, 14.0,
                hex_color(&self.cfg.text_color));
        }
    }
}

fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::from_rgba(c.red(), c.green(), c.blue(), (c.alpha() * alpha).clamp(0.0, 1.0))
        .unwrap_or(c)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let mut end = max;
    while !s.is_char_boundary(end) { end -= 1; }
    format!("{}...", &s[..end])
}

/// Blit RGBA icon pixels onto the pixmap with alpha blending.
fn blit_icon_rgba(pixmap: &mut Pixmap, x: i32, y: i32, size: u32, rgba: &[u8]) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let pixels = pixmap.pixels_mut();

    for dy in 0..size as i32 {
        for dx in 0..size as i32 {
            let px = x + dx;
            let py = y + dy;
            if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
            let si = (dy as usize * size as usize + dx as usize) * 4;
            if si + 3 >= rgba.len() { continue; }
            let (sr, sg, sb, sa) = (rgba[si], rgba[si + 1], rgba[si + 2], rgba[si + 3]);
            if sa == 0 { continue; }
            let idx = (py * pw + px) as usize;
            let dst = &mut pixels[idx];
            let src_a = sa as u16;
            let inv_a = 255u16 - src_a;
            let dr = ((sr as u16 * src_a + dst.red() as u16 * inv_a) / 255) as u8;
            let dg = ((sg as u16 * src_a + dst.green() as u16 * inv_a) / 255) as u8;
            let db = ((sb as u16 * src_a + dst.blue() as u16 * inv_a) / 255) as u8;
            let da = src_a.saturating_add(dst.alpha() as u16 * inv_a / 255) as u8;
            *dst = tiny_skia::PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
        }
    }
}

fn pixmap_to_bgra(pixmap: Pixmap) -> Vec<u8> {
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
