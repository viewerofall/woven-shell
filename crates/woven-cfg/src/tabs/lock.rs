//! Lock screen config tab.

use tiny_skia::Pixmap;
use crate::panel::*;

pub fn render(panel: &mut Panel, pm: &mut Pixmap, w: f32) {
    let font = &panel.text_font.clone();
    let mut y = 12.0f32;
    let pad  = 14.0f32;
    let fw   = (w - pad * 3.0) / 2.0;

    // Background type
    draw_text(pm, font, "Background", pad, y + 2.0, 10.5, DIM);
    let mut kx = pad + 90.0;
    for (id, label) in [("random", "Random"), ("image", "Fixed image")] {
        let selected = panel.lock_inputs.bg_kind == id;
        let bg = if selected { ACCENT } else { BORDER };
        let fg = if selected { BG } else { FG };
        let kw = draw_pill(pm, font, label, kx, y, 26.0, fg, bg);
        panel.zones.push(Zone { x0: kx, y0: y, x1: kx + kw, y1: y + 26.0,
                                action: ZoneAction::LockBgKindSelect(id.to_string()) });
        kx += kw + 6.0;
    }
    y += 36.0;

    let bg_kind = panel.lock_inputs.bg_kind.clone();
    if bg_kind == "random" {
        let f = panel.lock_focused == Some(LockField::Dir);
        draw_field(pm, font, "Wallpaper directory", &panel.lock_inputs.dir.value.clone(), f,
                   pad, y, w - pad * 2.0, 40.0);
        panel.zones.push(Zone { x0: pad, y0: y, x1: w - pad, y1: y + 40.0,
                                action: ZoneAction::LockFieldFocus(LockField::Dir) });
    } else {
        let f = panel.lock_focused == Some(LockField::Path);
        draw_field(pm, font, "Image path", &panel.lock_inputs.path.value.clone(), f,
                   pad, y, w - pad * 2.0, 40.0);
        panel.zones.push(Zone { x0: pad, y0: y, x1: w - pad, y1: y + 40.0,
                                action: ZoneAction::LockFieldFocus(LockField::Path) });
    }
    y += 48.0;

    // Blur radius
    let f = panel.lock_focused == Some(LockField::BlurRadius);
    draw_field(pm, font, "Blur radius", &panel.lock_inputs.blur_radius.value.clone(), f,
               pad, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad, y0: y, x1: pad + fw, y1: y + 40.0,
                            action: ZoneAction::LockFieldFocus(LockField::BlurRadius) });

    let f = panel.lock_focused == Some(LockField::FadeInMs);
    draw_field(pm, font, "Fade ms", &panel.lock_inputs.fade_in_ms.value.clone(), f,
               pad * 2.0 + fw, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad * 2.0 + fw, y0: y, x1: pad * 2.0 + fw * 2.0, y1: y + 40.0,
                            action: ZoneAction::LockFieldFocus(LockField::FadeInMs) });
    y += 48.0;

    // Clock/date
    section_header(pm, font, "Clock & Date", pad, y); y += 22.0;

    let f = panel.lock_focused == Some(LockField::ClockFormat);
    draw_field(pm, font, "Clock format (strftime)", &panel.lock_inputs.clock_format.value.clone(), f,
               pad, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad, y0: y, x1: pad + fw, y1: y + 40.0,
                            action: ZoneAction::LockFieldFocus(LockField::ClockFormat) });

    let f = panel.lock_focused == Some(LockField::DateFormat);
    draw_field(pm, font, "Date format (strftime)", &panel.lock_inputs.date_format.value.clone(), f,
               pad * 2.0 + fw, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad * 2.0 + fw, y0: y, x1: pad * 2.0 + fw * 2.0, y1: y + 40.0,
                            action: ZoneAction::LockFieldFocus(LockField::DateFormat) });
    y += 48.0;

    // Toggles row
    for (label, toggle, action) in [
        ("Show clock",    panel.lock_inputs.show_clock.value,  ZoneAction::LockToggle(LockToggleField::ShowClock)),
        ("Show date",     panel.lock_inputs.show_date.value,   ZoneAction::LockToggle(LockToggleField::ShowDate)),
        ("Shake on error",panel.lock_inputs.shake.value,       ZoneAction::LockToggle(LockToggleField::ShakeOnError)),
    ] {
        draw_text(pm, font, label, pad, y + 4.0, 12.0, FG);
        draw_toggle(pm, pad + 160.0, y + 1.0, toggle);
        panel.zones.push(Zone { x0: pad, y0: y, x1: pad + 200.0, y1: y + 22.0, action });
        y += 28.0;
    }
    y += 8.0;

    // Colors
    section_header(pm, font, "Colors", pad, y); y += 22.0;
    let fw3 = (w - pad * 4.0) / 3.0;

    for (label, field, val) in [
        ("Text color",   LockField::TextColor,   panel.lock_inputs.text_color.value.clone()),
        ("Accent color", LockField::AccentColor,  panel.lock_inputs.accent_color.value.clone()),
        ("Error color",  LockField::ErrorColor,   panel.lock_inputs.error_color.value.clone()),
    ] {
        let col_i = match field {
            LockField::TextColor   => 0,
            LockField::AccentColor => 1,
            LockField::ErrorColor  => 2,
            _ => 0,
        };
        let fx = pad + col_i as f32 * (fw3 + pad);
        if val.starts_with('#') { fill_rrect(pm, fx, y, 12.0, 12.0, 3.0, &val); }
        let f = panel.lock_focused == Some(field.clone());
        draw_field(pm, font, label, &val, f, fx, y, fw3, 40.0);
        panel.zones.push(Zone { x0: fx, y0: y, x1: fx + fw3, y1: y + 40.0,
                                action: ZoneAction::LockFieldFocus(field) });
    }
}

fn section_header(pm: &mut Pixmap, font: &fontdue::Font, label: &str, x: f32, y: f32) {
    draw_text(pm, font, label, x, y, 11.0, ACCENT);
    fill_rect(pm, x + measure(font, label, 11.0) + 8.0, y + 6.0, 400.0, 1.0, BORDER);
}

const ACCENT: &str = "#c792ea";
const FG:     &str = "#cdd6f4";
const DIM:    &str = "#6a508a";
const BORDER: &str = "#2a1545";
const BG:     &str = "#0a0010";
