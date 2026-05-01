//! Power menu panel — state machine + tiny-skia rendering.

use tiny_skia::*;

// ── Palette ──────────────────────────────────────────────────────���────────────
const BG_OVERLAY: (u8, u8, u8, u8) = (10, 0, 16, 210); // #0a0010 @ 82%
const BG_CARD:    &str = "#160026";
const BG_SEL:     &str = "#2a1045";
const ACCENT:     &str = "#c792ea";
const TEAL:       &str = "#00e5c8";
const FG:         &str = "#cdd6f4";
const DIM:        &str = "#4a3060";
const RED:        &str = "#f07178";
const BORDER:     &str = "#3a2060";
const BORDER_SEL: &str = "#c792ea";

// ── Actions ─────────────────────────────────────────────────────────��─────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Lock,
    Logout,
    Suspend,
    Hibernate,
    Reboot,
    Shutdown,
}

impl Action {
    const ALL: &'static [Action] = &[
        Action::Lock,
        Action::Logout,
        Action::Suspend,
        Action::Hibernate,
        Action::Reboot,
        Action::Shutdown,
    ];

    fn label(self) -> &'static str {
        match self {
            Action::Lock      => "Lock",
            Action::Logout    => "Logout",
            Action::Suspend   => "Suspend",
            Action::Hibernate => "Hibernate",
            Action::Reboot    => "Reboot",
            Action::Shutdown  => "Shutdown",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Action::Lock      => "\u{f033e}", // nf-md-lock
            Action::Logout    => "\u{f0343}", // nf-md-logout
            Action::Suspend   => "\u{f04b2}", // nf-md-sleep
            Action::Hibernate => "\u{f0e4d}", // nf-md-snowflake-melt (hibernate)
            Action::Reboot    => "\u{f0709}", // nf-md-restart
            Action::Shutdown  => "\u{f0425}", // nf-md-power
        }
    }

    fn color(self) -> &'static str {
        match self {
            Action::Lock | Action::Logout | Action::Suspend | Action::Hibernate => FG,
            Action::Reboot   => TEAL,
            Action::Shutdown => RED,
        }
    }

    /// Actions that require a confirmation step.
    fn needs_confirm(self) -> bool {
        matches!(self, Action::Reboot | Action::Shutdown | Action::Hibernate)
    }

    fn confirm_text(self) -> &'static str {
        match self {
            Action::Reboot    => "Reboot the system?",
            Action::Shutdown  => "Shut down the system?",
            Action::Hibernate => "Hibernate the system?",
            _                 => "Are you sure?",
        }
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Confirm(Action),
}

pub struct Panel {
    pub screen:    Screen,
    pub selected:  usize, // index into Action::ALL or confirm button (0=cancel 1=ok)
    text_font:     fontdue::Font,
    icon_font:     Option<fontdue::Font>,
}

impl Panel {
    pub fn new() -> Self {
        let text_font = load_font(&[
            "/usr/share/fonts/TTF/Inconsolata-Regular.ttf",
            "/usr/share/fonts/TTF/Inconsolata.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]).expect("no text font");

        let icon_font = load_font(&[
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
            "/usr/share/fonts/OTF/JetBrainsMonoNerdFont-Regular.otf",
            "/usr/share/fonts/TTF/FiraCodeNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/SymbolsNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/SymbolsNerdFontMono-Regular.ttf",
        ]);

        Self { screen: Screen::Main, selected: 0, text_font, icon_font }
    }

    // ── Keyboard navigation ───────────────────────────────────────────────────

    /// Returns true if the panel should close.
    pub fn key_escape(&mut self) -> bool {
        match self.screen {
            Screen::Confirm(_) => { self.screen = Screen::Main; false }
            Screen::Main       => true,
        }
    }

    pub fn key_enter(&mut self) -> bool {
        match self.screen {
            Screen::Main => {
                let action = Action::ALL[self.selected];
                self.activate(action)
            }
            Screen::Confirm(action) => {
                if self.selected == 1 { // OK
                    self.execute(action);
                    true
                } else { // Cancel
                    self.screen   = Screen::Main;
                    self.selected = 0;
                    false
                }
            }
        }
    }

    /// Returns true if panel should close.
    pub fn activate(&mut self, action: Action) -> bool {
        if action.needs_confirm() {
            self.screen   = Screen::Confirm(action);
            self.selected = 0; // 0=Cancel selected by default
            false
        } else {
            self.execute(action);
            true
        }
    }

    pub fn nav_prev(&mut self) {
        match self.screen {
            Screen::Main => {
                if self.selected > 0 { self.selected -= 1; }
                else { self.selected = Action::ALL.len() - 1; }
            }
            Screen::Confirm(_) => { self.selected = 0; }
        }
    }

    pub fn nav_next(&mut self) {
        match self.screen {
            Screen::Main => {
                self.selected = (self.selected + 1) % Action::ALL.len();
            }
            Screen::Confirm(_) => { self.selected = 1; }
        }
    }

    pub fn nav_up(&mut self) {
        if let Screen::Main = self.screen {
            let cols = 3usize;
            if self.selected >= cols { self.selected -= cols; }
        }
    }

    pub fn nav_down(&mut self) {
        if let Screen::Main = self.screen {
            let cols = 3usize;
            if self.selected + cols < Action::ALL.len() { self.selected += cols; }
        }
    }

    pub fn select_number(&mut self, n: usize) -> bool {
        if let Screen::Main = self.screen {
            if n < Action::ALL.len() {
                self.selected = n;
                return self.key_enter();
            }
        }
        false
    }

    fn execute(&self, action: Action) {
        match action {
            Action::Lock => {
                let cfg = woven_lock::config::LockConfig::load();
                let lock_program = cfg.lock.lock_program.as_str();

                let result = if lock_program == "swaylock" {
                    std::process::Command::new("swaylock").arg("-f").spawn()
                } else {
                    std::process::Command::new("woven-lock").spawn()
                        .or_else(|_| std::process::Command::new("swaylock").arg("-f").spawn())
                };

                if result.is_err() {
                    eprintln!("No lock program found, please install woven-lock or swaylock");
                }
            }
            Action::Logout => {
                // Try swaymsg first, then niri
                let _ = std::process::Command::new("swaymsg").arg("exit").spawn()
                    .or_else(|_| std::process::Command::new("niri").args(["msg", "action", "quit"]).spawn());
            }
            Action::Suspend   => { let _ = std::process::Command::new("systemctl").arg("suspend").spawn(); }
            Action::Hibernate => { let _ = std::process::Command::new("systemctl").arg("hibernate").spawn(); }
            Action::Reboot    => { let _ = std::process::Command::new("systemctl").arg("reboot").spawn(); }
            Action::Shutdown  => { let _ = std::process::Command::new("systemctl").arg("poweroff").spawn(); }
        }
    }

    // ── Rendering ────────────────────────────────────────────────────────────��

    /// Render the panel. Returns BGRA pixel bytes + click zones.
    pub fn render(&self, w: u32, h: u32) -> (Vec<u8>, Vec<ClickZone>) {
        let mut pm = Pixmap::new(w, h).unwrap();
        let (r, g, b, a) = BG_OVERLAY;
        pm.fill(Color::from_rgba8(r, g, b, a));

        let mut zones = Vec::new();

        match self.screen {
            Screen::Main        => self.draw_main(&mut pm, w, h, &mut zones),
            Screen::Confirm(ac) => self.draw_confirm(&mut pm, w, h, ac, &mut zones),
        }

        let data = pm.data();
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(4) {
            out.push(chunk[2]);
            out.push(chunk[1]);
            out.push(chunk[0]);
            out.push(chunk[3]);
        }
        (out, zones)
    }

    fn draw_main(&self, pm: &mut Pixmap, sw: u32, sh: u32, zones: &mut Vec<ClickZone>) {
        let btn_radius = 50.0f32;
        let gap        = 80.0f32;
        let n_actions  = Action::ALL.len() as f32;
        let total_w    = n_actions * btn_radius * 2.0 + (n_actions - 1.0) * gap;
        let ox         = (sw as f32 - total_w) / 2.0;
        let oy         = (sh as f32) / 2.0 - btn_radius; // center vertically

        // Hint line at bottom
        let hint = "1-6  ↑↓  Enter  Esc";
        let hw = self.measure_text(hint, 11.0);
        self.blit_text(pm, hint, (sw as f32 - hw) / 2.0, oy + btn_radius * 2.0 + 40.0, 11.0, hex(DIM));

        for (i, &action) in Action::ALL.iter().enumerate() {
            let cx = ox + i as f32 * (btn_radius * 2.0 + gap) + btn_radius;
            let cy = oy + btn_radius;

            let selected = i == self.selected;

            // Circle background — teal for selected, darker for unselected
            let bg_col = if selected { hex(TEAL) } else { hex(DIM) };
            draw_circle(pm, cx, cy, btn_radius, bg_col);

            // Selection ring for selected state
            if selected {
                draw_circle_stroke(pm, cx, cy, btn_radius, hex(ACCENT), 2.5);
            }

            // Icon in center
            let icon     = action.icon();
            let icon_sz  = 40.0f32;
            let icon_col = if selected { hex(BG_CARD) } else { hex(action.color()) };
            let iw       = self.measure_icon(icon, icon_sz);
            self.blit_icon(pm, icon, cx - iw / 2.0, cy - icon_sz / 2.0, icon_sz, icon_col);

            zones.push(ClickZone {
                action_idx: i,
                x0: cx - btn_radius,
                y0: cy - btn_radius,
                x1: cx + btn_radius,
                y1: cy + btn_radius,
            });
        }
    }

    fn draw_confirm(&self, pm: &mut Pixmap, sw: u32, sh: u32, action: Action, zones: &mut Vec<ClickZone>) {
        let dialog_w = 420.0f32;
        let dialog_h = 180.0f32;
        let dx = (sw as f32 - dialog_w) / 2.0;
        let dy = (sh as f32 - dialog_h) / 2.0;

        // Dialog box
        draw_rrect(pm, dx, dy, dialog_w, dialog_h, 14.0, hex(BG_CARD));
        draw_rrect_stroke(pm, dx, dy, dialog_w, dialog_h, 14.0, hex(BORDER_SEL), 1.5);

        // Icon + prompt text
        let icon    = action.icon();
        let icon_sz = 24.0f32;
        let iw      = self.measure_icon(icon, icon_sz);
        self.blit_icon(pm, icon, dx + (dialog_w - iw) / 2.0, dy + 28.0, icon_sz, hex(action.color()));

        let msg  = action.confirm_text();
        let mw   = self.measure_text(msg, 15.0);
        self.blit_text(pm, msg, dx + (dialog_w - mw) / 2.0, dy + 66.0, 15.0, hex(FG));

        // Cancel / Confirm buttons
        let btn_w = 140.0f32;
        let btn_h = 38.0f32;
        let btn_y = dy + dialog_h - btn_h - 18.0;
        let cancel_x  = dx + dialog_w / 2.0 - btn_w - 10.0;
        let confirm_x = dx + dialog_w / 2.0 + 10.0;

        // Cancel (index 0)
        let cancel_sel = self.selected == 0;
        let cancel_bg  = if cancel_sel { hex(DIM) } else { hex(BORDER) };
        draw_rrect(pm, cancel_x, btn_y, btn_w, btn_h, 8.0, cancel_bg);
        if cancel_sel { draw_rrect_stroke(pm, cancel_x, btn_y, btn_w, btn_h, 8.0, hex(BORDER_SEL), 1.5); }
        let cw = self.measure_text("Cancel", 13.0);
        self.blit_text(pm, "Cancel", cancel_x + (btn_w - cw) / 2.0, btn_y + 12.0, 13.0, hex(FG));

        // Confirm (index 1)
        let ok_sel = self.selected == 1;
        let ok_bg  = if ok_sel { hex(action.color()) } else { hex(BORDER) };
        draw_rrect(pm, confirm_x, btn_y, btn_w, btn_h, 8.0, ok_bg);
        if ok_sel { draw_rrect_stroke(pm, confirm_x, btn_y, btn_w, btn_h, 8.0, hex(BORDER_SEL), 1.5); }
        let ow = self.measure_text(action.label(), 13.0);
        let oc = if ok_sel { hex(BG_CARD) } else { hex(FG) };
        self.blit_text(pm, action.label(), confirm_x + (btn_w - ow) / 2.0, btn_y + 12.0, 13.0, oc);

        zones.push(ClickZone { action_idx: 100, x0: cancel_x,  y0: btn_y, x1: cancel_x  + btn_w, y1: btn_y + btn_h });
        zones.push(ClickZone { action_idx: 101, x0: confirm_x, y0: btn_y, x1: confirm_x + btn_w, y1: btn_y + btn_h });
    }

    // ── Font helpers ──────────────────────────────────────────────────────────

    fn pick_font<'a>(&'a self, ch: char) -> &'a fontdue::Font {
        let c = ch as u32;
        let is_icon = matches!(c, 0xE000..=0xF8FF | 0xF0000..=0xFFFFF | 0x100000..=0x10FFFF);
        if is_icon { self.icon_font.as_ref().unwrap_or(&self.text_font) }
        else       { &self.text_font }
    }

    fn measure_text(&self, s: &str, size: f32) -> f32 {
        s.chars().map(|c| self.text_font.metrics(c, size).advance_width).sum()
    }

    fn measure_icon(&self, s: &str, size: f32) -> f32 {
        let f = self.icon_font.as_ref().unwrap_or(&self.text_font);
        s.chars().map(|c| f.metrics(c, size).advance_width).sum()
    }

    fn blit_text(&self, pm: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let r  = (color.red()   * 255.0) as u8;
        let g  = (color.green() * 255.0) as u8;
        let b  = (color.blue()  * 255.0) as u8;
        let al = (color.alpha() * 255.0) as u8;
        if al == 0 { return; }
        let pw = pm.width() as i32;
        let ph = pm.height() as i32;
        let mut cx = x;
        for ch in text.chars() {
            let font = self.pick_font(ch);
            let (m, bm) = font.rasterize(ch, size);
            if m.width == 0 { cx += m.advance_width; continue; }
            let gx = (cx + m.xmin as f32).round() as i32;
            let gy = (y + size - m.height as f32 - m.ymin as f32).round() as i32;
            let pixels = pm.pixels_mut();
            for row in 0..m.height {
                for col in 0..m.width {
                    let cov = bm[row * m.width + col];
                    if cov == 0 { continue; }
                    let px = gx + col as i32;
                    let py = gy + row as i32;
                    if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
                    let idx   = (py * pw + px) as usize;
                    let dst   = &mut pixels[idx];
                    let sa    = (cov as u16 * al as u16 / 255) as u8;
                    let inv   = 255u16 - sa as u16;
                    let dr = ((r as u16 * sa as u16 / 255) + dst.red()   as u16 * inv / 255) as u8;
                    let dg = ((g as u16 * sa as u16 / 255) + dst.green() as u16 * inv / 255) as u8;
                    let db = ((b as u16 * sa as u16 / 255) + dst.blue()  as u16 * inv / 255) as u8;
                    let da = (sa as u16 + dst.alpha() as u16 * inv / 255).min(255) as u8;
                    *dst = PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
                }
            }
            cx += m.advance_width;
        }
    }

    fn blit_icon(&self, pm: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let font = self.icon_font.as_ref().unwrap_or(&self.text_font);
        let r  = (color.red()   * 255.0) as u8;
        let g  = (color.green() * 255.0) as u8;
        let b  = (color.blue()  * 255.0) as u8;
        let al = (color.alpha() * 255.0) as u8;
        if al == 0 { return; }
        let pw = pm.width() as i32;
        let ph = pm.height() as i32;
        let mut cx = x;
        for ch in text.chars() {
            let (m, bm) = font.rasterize(ch, size);
            if m.width == 0 { cx += m.advance_width; continue; }
            let gx = (cx + m.xmin as f32).round() as i32;
            let gy = (y + size - m.height as f32 - m.ymin as f32).round() as i32;
            let pixels = pm.pixels_mut();
            for row in 0..m.height {
                for col in 0..m.width {
                    let cov = bm[row * m.width + col];
                    if cov == 0 { continue; }
                    let px = gx + col as i32;
                    let py = gy + row as i32;
                    if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
                    let idx = (py * pw + px) as usize;
                    let dst = &mut pixels[idx];
                    let sa  = (cov as u16 * al as u16 / 255) as u8;
                    let inv = 255u16 - sa as u16;
                    let dr = ((r as u16 * sa as u16 / 255) + dst.red()   as u16 * inv / 255) as u8;
                    let dg = ((g as u16 * sa as u16 / 255) + dst.green() as u16 * inv / 255) as u8;
                    let db = ((b as u16 * sa as u16 / 255) + dst.blue()  as u16 * inv / 255) as u8;
                    let da = (sa as u16 + dst.alpha() as u16 * inv / 255).min(255) as u8;
                    *dst = PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
                }
            }
            cx += m.advance_width;
        }
    }
}

// ── Click zone ────────────────────────────────────────────────────────────────

/// action_idx: 0-5 = action cards; 100 = cancel; 101 = confirm button
#[derive(Clone)]
pub struct ClickZone {
    pub action_idx: usize,
    pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32,
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

fn hex(s: &str) -> Color {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0xFFFFFF);
    Color::from_rgba8((v >> 16) as u8, (v >> 8 & 0xFF) as u8, (v & 0xFF) as u8, 255)
}

fn paint_col(color: Color, aa: bool) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = aa;
    p
}

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    if w <= 0.0 || h <= 0.0 { return None; }
    if r <= 0.0 || w < r * 2.0 || h < r * 2.0 {
        return Rect::from_xywh(x, y, w, h).map(PathBuilder::from_rect);
    }
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

fn draw_rrect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if let Some(path) = rrect_path(x, y, w, h, r) {
        pm.fill_path(&path, &paint_col(color, true), FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_rrect_stroke(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color, width: f32) {
    if let Some(path) = rrect_path(x + 0.5, y + 0.5, w - 1.0, h - 1.0, r) {
        let mut stroke = Stroke::default();
        stroke.width = width;
        pm.stroke_path(&path, &paint_col(color, true), &stroke, Transform::identity(), None);
    }
}

fn draw_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color) {
    let mut pb = PathBuilder::new();
    // Approximate circle with bezier curves
    let k = 0.55228475; // magic constant for circular bezier approximation
    let r_k = r * k;
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + r_k, cx + r_k, cy + r, cx, cy + r);
    pb.cubic_to(cx - r_k, cy + r, cx - r, cy + r_k, cx - r, cy);
    pb.cubic_to(cx - r, cy - r_k, cx - r_k, cy - r, cx, cy - r);
    pb.cubic_to(cx + r_k, cy - r, cx + r, cy - r_k, cx + r, cy);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint_col(color, true), FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_circle_stroke(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
    let mut pb = PathBuilder::new();
    let k = 0.55228475;
    let r_k = r * k;
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + r_k, cx + r_k, cy + r, cx, cy + r);
    pb.cubic_to(cx - r_k, cy + r, cx - r, cy + r_k, cx - r, cy);
    pb.cubic_to(cx - r, cy - r_k, cx - r_k, cy - r, cx, cy - r);
    pb.cubic_to(cx + r_k, cy - r, cx + r, cy - r_k, cx + r, cy);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut stroke = Stroke::default();
        stroke.width = width;
        pm.stroke_path(&path, &paint_col(color, true), &stroke, Transform::identity(), None);
    }
}

fn load_font(paths: &[&str]) -> Option<fontdue::Font> {
    for p in paths {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(f) = fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()) {
                return Some(f);
            }
        }
    }
    None
}
