//! Theme switcher tab.

use tiny_skia::Pixmap;
use crate::panel::*;
use std::fs;

pub fn render(panel: &mut Panel, pm: &mut Pixmap, w: f32) {
    let font = &panel.text_font.clone();
    let mut y = 12.0f32;
    let pad = 14.0f32;

    // Load available themes from ~/.config/woven-shell/themes/
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let themes_dir = format!("{}/.config/woven-shell/themes", home);

    let mut theme_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".toml") {
                    theme_files.push(name.trim_end_matches(".toml").to_string());
                }
            }
        }
    }
    theme_files.sort();

    // Title
    draw_text(pm, font, "Available themes", pad, y + 2.0, 10.5, DIM);
    y += 28.0;

    // Theme buttons
    let mut tx = pad;
    for theme in theme_files {
        let is_selected = panel.selected_theme == theme;
        let bg = if is_selected { ACCENT } else { BORDER };
        let fg = if is_selected { BG } else { FG };
        let tw = draw_pill(pm, font, &theme, tx, y, 26.0, fg, bg);
        panel.zones.push(Zone {
            x0: tx, y0: y, x1: tx + tw, y1: y + 26.0,
            action: ZoneAction::ThemeSelect(theme),
        });
        tx += tw + 8.0;
        if tx > w - pad * 2.0 {
            tx = pad;
            y += 34.0;
        }
    }

    y += 36.0;
    draw_text(pm, font, "Themes are applied globally when you click Apply", pad, y + 2.0, 9.5, DIM);
}

const ACCENT: &str = "#c792ea";
const FG:     &str = "#cdd6f4";
const DIM:    &str = "#6a508a";
const BORDER: &str = "#2a1545";
const BG:     &str = "#0a0010";
