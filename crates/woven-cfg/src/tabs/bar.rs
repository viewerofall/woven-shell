//! Bar config tab.

use tiny_skia::Pixmap;
use crate::panel::*;

const ALL_MODULES: &[&str] = &[
    "activities", "workspaces", "window_title", "clock", "network", "audio",
    "battery", "systray", "cpu", "memory", "disk", "temp", "media",
    "notifications", "weather", "control_center", "|",
];

pub fn render(panel: &mut Panel, pm: &mut Pixmap, w: f32) {
    let font = &panel.text_font.clone();
    let mut y = 12.0f32;
    let pad  = 14.0f32;
    let fw   = (w - pad * 3.0) / 2.0;

    // ── Module lists ──────────────────────────────────────────────────────────
    section_header(pm, font, "Modules", pad, y); y += 22.0;

    for (slot_name, slot, modules) in [
        ("Left",   BarSlot::Left,   panel.cfg.bar.modules.left.clone()),
        ("Center", BarSlot::Center, panel.cfg.bar.modules.center.clone()),
        ("Right",  BarSlot::Right,  panel.cfg.bar.modules.right.clone()),
    ] {
        draw_text(pm, font, slot_name, pad, y + 4.0, 10.5, DIM);
        let mut mx = pad + 52.0;
        for (i, m) in modules.iter().enumerate() {
            let col = if m == "|" { DIM } else { TEAL };
            let bg  = if m == "|" { "#2a1545" } else { "#0d2030" };
            let pw  = draw_pill(pm, font, m, mx, y, 24.0, col, bg);
            // × button
            let xb = mx + pw + 1.0;
            draw_text(pm, font, "×", xb, y + 4.0, 11.0, RED);
            let xa = ZoneAction::BarModuleRemove { slot: slot.clone(), idx: i };
            panel.zones.push(Zone { x0: mx, y0: y, x1: xb + 10.0, y1: y + 24.0, action: xa });
            mx = xb + 14.0;
            if mx > w - pad - 40.0 { mx = pad + 52.0; y += 26.0; }
        }
        // + add
        fill_rrect(pm, mx, y + 2.0, 28.0, 20.0, 5.0, BORDER);
        draw_text(pm, font, "+ ", mx + 7.0, y + 5.0, 11.0, ACCENT);
        panel.zones.push(Zone { x0: mx, y0: y + 2.0, x1: mx + 28.0, y1: y + 22.0,
                                action: ZoneAction::BarModuleAdd(slot) });
        y += 32.0;
    }

    // Module picker overlay
    if let Some(ref slot) = panel.bar_module_picker.clone() {
        let picker_x = pad;
        let picker_y = y;
        fill_rrect(pm, picker_x, picker_y, w - pad * 2.0, 80.0, 8.0, "#160030");
        stroke_rrect(pm, picker_x, picker_y, w - pad * 2.0, 80.0, 8.0, ACCENT);
        draw_text(pm, font, "Pick module:", picker_x + 8.0, picker_y + 6.0, 10.5, DIM);
        let mut mx = picker_x + 8.0;
        let mut my = picker_y + 22.0;
        for m in ALL_MODULES {
            let mw = draw_pill(pm, font, m, mx, my, 22.0, TEAL, "#0d2030");
            panel.zones.push(Zone { x0: mx, y0: my, x1: mx + mw, y1: my + 22.0,
                                    action: ZoneAction::BarModulePicker(m.to_string()) });
            mx += mw + 6.0;
            if mx > w - pad - 60.0 { mx = picker_x + 8.0; my += 26.0; }
        }
        panel.zones.push(Zone { x0: picker_x, y0: picker_y, x1: picker_x + w - pad * 2.0, y1: picker_y + 80.0,
                                action: ZoneAction::BarModulePickerClose });
        y += 88.0;
    }

    // ── Basic settings ────────────────────────────────────────────────────────
    section_header(pm, font, "Settings", pad, y); y += 22.0;

    let h_focused = panel.bar_focused == Some(BarField::Height);
    draw_field(pm, font, "Height (px)", &panel.bar_inputs.height.value.clone(),
               h_focused, pad, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad, y0: y, x1: pad + fw, y1: y + 40.0,
                            action: ZoneAction::BarFieldFocus(BarField::Height) });

    let p_focused = panel.bar_focused == Some(BarField::Position);
    draw_field(pm, font, "Position (top/bottom)", &panel.bar_inputs.position.value.clone(),
               p_focused, pad * 2.0 + fw, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad * 2.0 + fw, y0: y, x1: pad * 2.0 + fw * 2.0, y1: y + 40.0,
                            action: ZoneAction::BarFieldFocus(BarField::Position) });
    y += 52.0;

    // ── Style toggles ─────────────────────────────────────────────────────────
    for (label, val, action) in [
        ("Bubbles style",     panel.bar_inputs.use_bubbles.value,     ZoneAction::BarToggle(BarToggleField::Bubbles)),
        ("Wallpaper theme",   panel.bar_inputs.wallpaper_theme.value, ZoneAction::BarToggle(BarToggleField::WallpaperTheme)),
    ] {
        draw_text(pm, font, label, pad, y + 4.0, 12.0, FG);
        draw_toggle(pm, pad + 160.0, y + 1.0, val);
        panel.zones.push(Zone { x0: pad, y0: y, x1: pad + 200.0, y1: y + 22.0, action });
        y += 28.0;
    }
    let _ = y;
}

fn section_header(pm: &mut Pixmap, font: &fontdue::Font, label: &str, x: f32, y: f32) {
    draw_text(pm, font, label, x, y, 11.0, ACCENT);
    fill_rect(pm, x + measure(font, label, 11.0) + 8.0, y + 6.0, 400.0, 1.0, BORDER);
}

const ACCENT: &str = "#c792ea";
const TEAL:   &str = "#00e5c8";
const FG:     &str = "#cdd6f4";
const DIM:    &str = "#6a508a";
const BORDER: &str = "#2a1545";
const RED:    &str = "#f07178";
