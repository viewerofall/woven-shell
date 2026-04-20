//! Wallpaper config tab.

use tiny_skia::Pixmap;
use crate::panel::*;

const KINDS: &[(&str, &str)] = &[
    ("image",    "Image"),
    ("gif",      "GIF"),
    ("video",    "Video"),
    ("color",    "Color"),
    ("gradient", "Gradient"),
    ("slideshow","Slideshow"),
];

pub fn render(panel: &mut Panel, pm: &mut Pixmap, w: f32) {
    let font = &panel.text_font.clone();
    let mut y = 12.0f32;
    let pad  = 14.0f32;
    let fw   = (w - pad * 3.0) / 2.0;

    // Type selector
    draw_text(pm, font, "Wallpaper type", pad, y + 2.0, 10.5, DIM);
    let mut kx = pad + 110.0;
    for (id, label) in KINDS {
        let selected = panel.wall_inputs.kind == *id;
        let bg = if selected { ACCENT } else { BORDER };
        let fg = if selected { BG } else { FG };
        let kw = draw_pill(pm, font, label, kx, y, 26.0, fg, bg);
        panel.zones.push(Zone { x0: kx, y0: y, x1: kx + kw, y1: y + 26.0,
                                action: ZoneAction::WallKindSelect(id.to_string()) });
        kx += kw + 6.0;
    }
    y += 36.0;

    let kind = panel.wall_inputs.kind.clone();

    match kind.as_str() {
        "image" | "gif" | "video" => {
            let f = panel.wall_focused == Some(WallField::Path);
            draw_field(pm, font, "Path", &panel.wall_inputs.path.value.clone(), f, pad, y, w - pad * 2.0, 40.0);
            panel.zones.push(Zone { x0: pad, y0: y, x1: w - pad, y1: y + 40.0,
                                    action: ZoneAction::WallFieldFocus(WallField::Path) });
            y += 48.0;
        }
        "color" => {
            let f = panel.wall_focused == Some(WallField::Color);
            let val = panel.wall_inputs.color.value.clone();
            if val.starts_with('#') { fill_rrect(pm, pad, y, 16.0, 16.0, 4.0, &val); }
            draw_field(pm, font, "Color (#rrggbb)", &val, f, pad, y, fw, 40.0);
            panel.zones.push(Zone { x0: pad, y0: y, x1: pad + fw, y1: y + 40.0,
                                    action: ZoneAction::WallFieldFocus(WallField::Color) });
            y += 48.0;
        }
        "slideshow" => {
            let fd = panel.wall_focused == Some(WallField::Dir);
            draw_field(pm, font, "Directory", &panel.wall_inputs.dir.value.clone(), fd, pad, y, w - pad * 2.0, 40.0);
            panel.zones.push(Zone { x0: pad, y0: y, x1: w - pad, y1: y + 40.0,
                                    action: ZoneAction::WallFieldFocus(WallField::Dir) });
            y += 48.0;

            let fi = panel.wall_focused == Some(WallField::Interval);
            draw_field(pm, font, "Interval (seconds)", &panel.wall_inputs.interval.value.clone(), fi, pad, y, fw, 40.0);
            panel.zones.push(Zone { x0: pad, y0: y, x1: pad + fw, y1: y + 40.0,
                                    action: ZoneAction::WallFieldFocus(WallField::Interval) });

            let ft = panel.wall_focused == Some(WallField::TransitionSecs);
            draw_field(pm, font, "Transition secs", &panel.wall_inputs.transition_secs.value.clone(), ft,
                       pad * 2.0 + fw, y, fw, 40.0);
            panel.zones.push(Zone { x0: pad * 2.0 + fw, y0: y, x1: pad * 2.0 + fw * 2.0, y1: y + 40.0,
                                    action: ZoneAction::WallFieldFocus(WallField::TransitionSecs) });
            y += 48.0;

            // Transition picker
            draw_text(pm, font, "Transition", pad, y + 2.0, 10.5, DIM);
            let mut tx = pad + 90.0;
            for kind in ["fade", "wipe", "slide", "pixelate"] {
                let selected = panel.cfg.wall.wallpaper.transition == kind;
                let bg = if selected { TEAL } else { BORDER };
                let fg = if selected { BG } else { FG };
                let tw = draw_pill(pm, font, kind, tx, y, 24.0, fg, bg);
                panel.zones.push(Zone { x0: tx, y0: y, x1: tx + tw, y1: y + 24.0,
                                        action: ZoneAction::WallKindSelect(kind.to_string()) });
                tx += tw + 6.0;
            }
        }
        "gradient" => {
            draw_text(pm, font, "Gradient colors (edit keybinds.toml directly for now)", pad, y + 8.0, 11.0, DIM);
        }
        _ => {}
    }
}

const ACCENT: &str = "#c792ea";
const TEAL:   &str = "#00e5c8";
const FG:     &str = "#cdd6f4";
const DIM:    &str = "#6a508a";
const BORDER: &str = "#2a1545";
const BG:     &str = "#0a0010";
