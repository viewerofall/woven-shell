//! OSD display state and animation phases.

use std::time::{Duration, Instant};
use crate::read::{MediaState, VolumeState};

const HOLD_MS:  u64 = 1800;
const ENTER_MS: u64 = 120;
const EXIT_MS:  u64 = 250;

#[derive(Debug, Clone)]
pub enum OsdKind {
    Volume(VolumeState),
    Brightness(u8),
    Media(MediaState),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase { Enter, Hold, Exit, Hidden }

pub struct OsdState {
    pub kind:       Option<OsdKind>,
    pub phase:      Phase,
    phase_start:    Instant,
    pub alpha:      f32,   // 0.0–1.0 for fade
    pub offset_y:   f32,   // pixels offset downward (slide up on enter)
}

impl OsdState {
    pub fn new() -> Self {
        Self {
            kind: None, phase: Phase::Hidden,
            phase_start: Instant::now(),
            alpha: 0.0, offset_y: 20.0,
        }
    }

    pub fn show(&mut self, kind: OsdKind) {
        self.kind        = Some(kind);
        self.phase       = Phase::Enter;
        self.phase_start = Instant::now();
    }

    /// Advance animation. Returns true if a repaint is needed.
    pub fn tick(&mut self) -> bool {
        match self.phase {
            Phase::Hidden => false,
            Phase::Enter => {
                let t = self.elapsed_frac(ENTER_MS);
                self.alpha    = ease_out(t);
                self.offset_y = 20.0 * (1.0 - ease_out(t));
                if t >= 1.0 {
                    self.alpha    = 1.0;
                    self.offset_y = 0.0;
                    self.phase    = Phase::Hold;
                    self.phase_start = Instant::now();
                }
                true
            }
            Phase::Hold => {
                if self.phase_start.elapsed() >= Duration::from_millis(HOLD_MS) {
                    self.phase       = Phase::Exit;
                    self.phase_start = Instant::now();
                    return true;
                }
                false
            }
            Phase::Exit => {
                let t = self.elapsed_frac(EXIT_MS);
                self.alpha    = 1.0 - ease_in(t);
                self.offset_y = 20.0 * ease_in(t);
                if t >= 1.0 {
                    self.alpha  = 0.0;
                    self.phase  = Phase::Hidden;
                    self.kind   = None;
                }
                true
            }
        }
    }

    pub fn visible(&self) -> bool { self.phase != Phase::Hidden }

    fn elapsed_frac(&self, total_ms: u64) -> f32 {
        let elapsed = self.phase_start.elapsed().as_millis() as f32;
        (elapsed / total_ms as f32).min(1.0)
    }
}

fn ease_out(t: f32) -> f32 { 1.0 - (1.0 - t).powi(2) }
fn ease_in(t: f32)  -> f32 { t * t }
