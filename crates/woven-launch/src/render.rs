//! Launcher renderer — floating centered panel with search, results, calculator.

use crate::calc;
use crate::config::LauncherSettings;
use crate::desktop::DesktopEntry;
use crate::draw::*;
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

    pub text: TextRenderer,
    cursor_blink: Instant,
    pub dirty: bool,
    cfg: LauncherSettings,
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
            text: TextRenderer::new(),
            cursor_blink: Instant::now(),
            dirty: true,
            cfg,
        }
    }

    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.text.clear_dynamic();
        self.refilter();
        self.dirty = true;
        self.cursor_blink = Instant::now();
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
        self.text.clear_dynamic();
        self.refilter();
        self.dirty = true;
        self.cursor_blink = Instant::now();
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.text.clear_dynamic();
        self.refilter();
        self.dirty = true;
    }

    pub fn select_up(&mut self) {
        if self.selected > 0 { self.selected -= 1; }
        self.dirty = true;
    }

    pub fn select_down(&mut self) {
        let max = self.visible_count().saturating_sub(1);
        if self.selected < max { self.selected += 1; }
        self.dirty = true;
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
            self.results.len().min(self.cfg.max_results)
        }
    }

    fn refilter(&mut self) {
        if self.is_calc_mode() || self.is_cmd_mode() {
            self.results.clear();
            self.selected = 0;
            return;
        }
        self.results = search::fuzzy_search(&self.entries, &self.query);
        self.selected = 0;
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
        let mut pixmap = Pixmap::new(w, h).expect("pixmap");

        // 1. Dim backdrop
        fill_rect(&mut pixmap, 0.0, 0.0, w as f32, h as f32,
            Color::from_rgba8(0x0a, 0x00, 0x10, BACKDROP_ALPHA));

        let panel_w = self.cfg.width as f32;
        let panel_h = self.panel_height();
        let px = (w as f32 - panel_w) / 2.0;
        let py = (h as f32 - panel_h) / 2.0 - 40.0; // slightly above center

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
            "= calculator mode    Esc close"
        } else if self.is_cmd_mode() {
            "↵ run command    Esc close"
        } else {
            "↑↓ navigate    ↵ launch    Esc close"
        };
        let hw = self.text.measure(hints, 12.0);
        self.text.draw(&mut pixmap, hints,
            px + (panel_w - hw) / 2.0, footer_y + 10.0, 12.0, hint_color);

        pixmap_to_bgra(pixmap)
    }

    fn render_results(&mut self, pixmap: &mut Pixmap, x: f32, mut y: f32, w: f32) {
        let max = self.cfg.max_results.min(self.results.len());

        for i in 0..max {
            let result = &self.results[i];
            let entry = &self.entries[result.index];
            let is_sel = i == self.selected;

            // selection highlight
            if is_sel {
                fill_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
                    hex_color(&self.cfg.selection_color));
                stroke_rounded_rect(pixmap, x, y, w, RESULT_H, RESULT_R,
                    with_alpha(hex_color(&self.cfg.accent_color), 0.3), 1.0);
            }

            // app initial circle
            let circle_x = x + 28.0;
            let circle_y = y + RESULT_H / 2.0;
            let circle_color = if is_sel {
                with_alpha(hex_color(&self.cfg.accent_color), 0.8)
            } else {
                with_alpha(hex_color(&self.cfg.accent_color), 0.3)
            };
            fill_circle(pixmap, circle_x, circle_y, 14.0, circle_color);

            // initial letter
            let initial = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
            let iw = self.text.measure(&initial, 14.0);
            self.text.draw(pixmap, &initial,
                circle_x - iw / 2.0, circle_y - 8.0, 14.0,
                hex_color(&self.cfg.text_color));

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
        let cmd = &self.query[1..].trim();

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
            self.text.draw(pixmap, cmd, label_x + prompt_w, label_y, 14.0,
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
