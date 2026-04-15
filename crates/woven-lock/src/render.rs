//! Lock screen renderer — clock, password field, animations.
//!
//! States: Idle → Typing → Verifying → Error (shake) → Unlocking (fade out)
//!         All states render over the blurred wallpaper background.

use crate::config::LockSettings;
use crate::draw::{fill_circle, fill_rect, fill_rounded_rect, hex_color, stroke_rounded_rect};
use crate::text::TextRenderer;
use chrono::Local;
use std::time::Instant;
use tiny_skia::{Color, Pixmap};

// ─── UI constants ────────────────────────────────────────────────────────────

const CLOCK_SIZE: f32 = 96.0;
const DATE_SIZE: f32 = 22.0;
const INPUT_SIZE: f32 = 18.0;
const DOT_RADIUS: f32 = 6.0;
const DOT_GAP: f32 = 18.0;
const FIELD_W: f32 = 340.0;
const FIELD_H: f32 = 52.0;
const FIELD_R: f32 = 26.0; // pill shape
const OVERLAY_ALPHA: u8 = 0x60; // dark overlay on blurred bg

// ─── Animation state ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum LockPhase {
    FadeIn,
    Idle,
    Typing,
    Verifying,
    Error,
    Unlocking,
}

pub struct LockRenderer {
    pub phase: LockPhase,
    pub password: String,
    pub text: TextRenderer,

    // animation timers
    phase_start: Instant,
    error_start: Option<Instant>,
    cursor_blink: Instant,

    // cached bg per output size
    bg_cache: Option<(u32, u32, Vec<u8>)>,

    // config
    cfg: LockSettings,
}

impl LockRenderer {
    pub fn new(cfg: LockSettings) -> Self {
        Self {
            phase: LockPhase::FadeIn,
            password: String::new(),
            text: TextRenderer::new(),
            phase_start: Instant::now(),
            error_start: None,
            cursor_blink: Instant::now(),
            bg_cache: None,
            cfg,
        }
    }

    pub fn set_background(&mut self, w: u32, h: u32, bgra: Vec<u8>) {
        self.bg_cache = Some((w, h, bgra));
    }

    pub fn push_char(&mut self, ch: char) {
        self.password.push(ch);
        self.phase = LockPhase::Typing;
        self.cursor_blink = Instant::now();
    }

    pub fn pop_char(&mut self) {
        self.password.pop();
        if self.password.is_empty() {
            self.phase = LockPhase::Idle;
        }
        self.cursor_blink = Instant::now();
    }

    pub fn clear_password(&mut self) {
        self.password.clear();
        self.phase = LockPhase::Idle;
    }

    pub fn start_verify(&mut self) {
        self.phase = LockPhase::Verifying;
        self.phase_start = Instant::now();
    }

    pub fn show_error(&mut self) {
        self.phase = LockPhase::Error;
        self.error_start = Some(Instant::now());
        self.password.clear();
    }

    pub fn start_unlock(&mut self) {
        self.phase = LockPhase::Unlocking;
        self.phase_start = Instant::now();
    }

    pub fn unlock_done(&self) -> bool {
        self.phase == LockPhase::Unlocking
            && self.phase_start.elapsed().as_millis() >= self.cfg.fade_out_ms as u128
    }

    /// Check if the error animation has finished and reset to idle.
    pub fn maybe_reset_error(&mut self) {
        if self.phase == LockPhase::Error {
            if let Some(start) = self.error_start {
                if start.elapsed().as_millis() > 800 {
                    self.phase = LockPhase::Idle;
                    self.error_start = None;
                }
            }
        }
    }

    pub fn is_animating(&self) -> bool {
        match self.phase {
            LockPhase::FadeIn => self.phase_start.elapsed().as_millis() < self.cfg.fade_in_ms as u128,
            LockPhase::Error => {
                self.error_start
                    .map(|t| t.elapsed().as_millis() < 600)
                    .unwrap_or(false)
            }
            LockPhase::Unlocking => !self.unlock_done(),
            LockPhase::Verifying => true,
            _ => {
                // cursor blink
                true
            }
        }
    }

    /// Render the lock screen. Returns BGRA pixels.
    pub fn render(&mut self, w: u32, h: u32) -> Vec<u8> {
        let mut pixmap = Pixmap::new(w, h).expect("pixmap");

        // 0. Fill fully opaque so nothing bleeds through
        fill_rect(&mut pixmap, 0.0, 0.0, w as f32, h as f32, Color::from_rgba8(0, 0, 0, 255));

        // 1. Draw blurred wallpaper background (or solid fallback)
        self.draw_background(&mut pixmap, w, h);

        // 2. Dark overlay for contrast
        fill_rect(
            &mut pixmap,
            0.0, 0.0, w as f32, h as f32,
            Color::from_rgba8(0, 0, 0, OVERLAY_ALPHA),
        );

        // 3. Fade-in effect
        let fade_alpha = match self.phase {
            LockPhase::FadeIn => {
                let t = self.phase_start.elapsed().as_millis() as f32 / self.cfg.fade_in_ms as f32;
                let a = t.min(1.0);
                if a >= 1.0 { self.phase = LockPhase::Idle; }
                a
            }
            LockPhase::Unlocking => {
                let t = self.phase_start.elapsed().as_millis() as f32 / self.cfg.fade_out_ms as f32;
                1.0 - t.min(1.0)
            }
            _ => 1.0,
        };

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;

        // 4. Clock
        if self.cfg.show_clock {
            let time_str = Local::now().format(&self.cfg.clock_format).to_string();
            let tw = self.text.measure(&time_str, CLOCK_SIZE);
            let tx = cx - tw / 2.0;
            let ty = cy - 120.0;
            let color = with_alpha(hex_color(&self.cfg.text_color), fade_alpha);
            self.text.draw(&mut pixmap, &time_str, tx, ty, CLOCK_SIZE, color);
        }

        // 5. Date
        if self.cfg.show_date {
            let date_str = Local::now().format(&self.cfg.date_format).to_string();
            let dw = self.text.measure(&date_str, DATE_SIZE);
            let dx = cx - dw / 2.0;
            let dy = cy - 30.0;
            let color = with_alpha(hex_color(&self.cfg.text_color), fade_alpha * 0.7);
            self.text.draw(&mut pixmap, &date_str, dx, dy, DATE_SIZE, color);
        }

        // 6. Password field
        let shake_offset = self.shake_offset();
        let field_x = cx - FIELD_W / 2.0 + shake_offset;
        let field_y = cy + 30.0;

        // field background (glass effect)
        fill_rounded_rect(
            &mut pixmap,
            field_x, field_y, FIELD_W, FIELD_H, FIELD_R,
            Color::from_rgba8(255, 255, 255, (20.0 * fade_alpha) as u8),
        );

        // field border
        let border_color = match self.phase {
            LockPhase::Error => with_alpha(hex_color(&self.cfg.error_color), fade_alpha),
            LockPhase::Verifying => with_alpha(hex_color(&self.cfg.accent_color), fade_alpha * 0.5),
            _ => with_alpha(hex_color(&self.cfg.accent_color), fade_alpha * 0.6),
        };
        stroke_rounded_rect(
            &mut pixmap,
            field_x, field_y, FIELD_W, FIELD_H, FIELD_R,
            border_color, 1.5,
        );

        // password dots or placeholder
        if self.password.is_empty() {
            let placeholder = match self.phase {
                LockPhase::Verifying => "verifying...",
                LockPhase::Error => "try again",
                _ => "password",
            };
            let pw = self.text.measure(placeholder, INPUT_SIZE);
            let px = cx - pw / 2.0 + shake_offset;
            let py = field_y + (FIELD_H - INPUT_SIZE) / 2.0;
            let ph_color = match self.phase {
                LockPhase::Error => with_alpha(hex_color(&self.cfg.error_color), fade_alpha * 0.8),
                _ => with_alpha(hex_color(&self.cfg.text_color), fade_alpha * 0.35),
            };
            self.text.draw(&mut pixmap, placeholder, px, py, INPUT_SIZE, ph_color);
        } else {
            let n = self.password.len() as f32;
            let total_w = n * DOT_RADIUS * 2.0 + (n - 1.0) * (DOT_GAP - DOT_RADIUS * 2.0);
            let start_x = cx - total_w / 2.0 + DOT_RADIUS + shake_offset;
            let dot_cy = field_y + FIELD_H / 2.0;
            let dot_color = with_alpha(hex_color(&self.cfg.accent_color), fade_alpha);

            for i in 0..self.password.len() {
                let dx = start_x + i as f32 * DOT_GAP;
                fill_circle(&mut pixmap, dx, dot_cy, DOT_RADIUS, dot_color);
            }
        }

        // 7. Cursor blink (thin line after last dot / in empty field)
        if matches!(self.phase, LockPhase::Idle | LockPhase::Typing) {
            let blink_ms = self.cursor_blink.elapsed().as_millis() % 1000;
            if blink_ms < 500 {
                let cursor_x = if self.password.is_empty() {
                    cx + self.text.measure("password", INPUT_SIZE) / 2.0 + 4.0
                } else {
                    let n = self.password.len() as f32;
                    let total_w = n * DOT_RADIUS * 2.0 + (n - 1.0) * (DOT_GAP - DOT_RADIUS * 2.0);
                    cx + total_w / 2.0 + 8.0
                };
                let cursor_color = with_alpha(hex_color(&self.cfg.accent_color), fade_alpha * 0.8);
                fill_rect(
                    &mut pixmap,
                    cursor_x, field_y + 12.0,
                    2.0, FIELD_H - 24.0,
                    cursor_color,
                );
            }
        }

        // 8. Subtle hint at bottom
        {
            let hint = "esc to clear";
            let hw = self.text.measure(hint, 13.0);
            let hx = cx - hw / 2.0;
            let hy = field_y + FIELD_H + 20.0;
            let hint_color = with_alpha(hex_color(&self.cfg.text_color), fade_alpha * 0.2);
            self.text.draw(&mut pixmap, hint, hx, hy, 13.0, hint_color);
        }

        pixmap.take()
    }

    fn draw_background(&self, pixmap: &mut Pixmap, w: u32, h: u32) {
        if let Some((bw, bh, ref bgra)) = self.bg_cache {
            if bw == w && bh == h && bgra.len() == (w * h * 4) as usize {
                // copy BGRA bytes directly — force fully opaque so no
                // semi-transparent pixels leave holes for stale data
                let data = pixmap.data_mut();
                data.copy_from_slice(bgra);
                for chunk in data.chunks_exact_mut(4) {
                    chunk[3] = 255; // force opaque
                }
                return;
            }
        }
        // fallback: solid dark
        let bg = Color::from_rgba8(0x0a, 0x00, 0x10, 0xff);
        fill_rect(pixmap, 0.0, 0.0, w as f32, h as f32, bg);
    }

    fn shake_offset(&self) -> f32 {
        if !self.cfg.shake_on_error || self.phase != LockPhase::Error {
            return 0.0;
        }
        let Some(start) = self.error_start else { return 0.0; };
        let elapsed_ms = start.elapsed().as_millis() as f32;
        if elapsed_ms > 500.0 { return 0.0; }

        // damped sine wave
        let t = elapsed_ms / 500.0;
        let decay = 1.0 - t;
        let freq = 6.0 * std::f32::consts::PI * t;
        decay * 12.0 * freq.sin()
    }
}

fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::from_rgba(c.red(), c.green(), c.blue(), (c.alpha() * alpha).clamp(0.0, 1.0))
        .unwrap_or(c)
}
