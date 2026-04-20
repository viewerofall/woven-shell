//! Launcher config tab.

use tiny_skia::Pixmap;
use crate::panel::*;

pub fn render(panel: &mut Panel, pm: &mut Pixmap, w: f32) {
    let font = &panel.text_font.clone();
    let mut y = 12.0f32;
    let pad  = 14.0f32;
    let fw   = (w - pad * 3.0) / 2.0;
    let fw3  = (w - pad * 4.0) / 3.0;

    section_header(pm, font, "Size", pad, y); y += 22.0;

    let f = panel.launch_focused == Some(LaunchField::Width);
    draw_field(pm, font, "Width (px)", &panel.launch_inputs.width.value.clone(), f,
               pad, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad, y0: y, x1: pad + fw, y1: y + 40.0,
                            action: ZoneAction::LaunchFieldFocus(LaunchField::Width) });

    let f = panel.launch_focused == Some(LaunchField::MaxResults);
    draw_field(pm, font, "Max results", &panel.launch_inputs.max_results.value.clone(), f,
               pad * 2.0 + fw, y, fw, 40.0);
    panel.zones.push(Zone { x0: pad * 2.0 + fw, y0: y, x1: pad * 2.0 + fw * 2.0, y1: y + 40.0,
                            action: ZoneAction::LaunchFieldFocus(LaunchField::MaxResults) });
    y += 48.0;

    section_header(pm, font, "Features", pad, y); y += 22.0;

    for (label, val, action) in [
        ("Calculator (= prefix)",   panel.launch_inputs.calculator.value,  ZoneAction::LaunchToggle(LaunchToggleField::Calculator)),
        ("Command runner (! prefix)",panel.launch_inputs.cmd_runner.value, ZoneAction::LaunchToggle(LaunchToggleField::CommandRunner)),
    ] {
        draw_text(pm, font, label, pad, y + 4.0, 12.0, FG);
        draw_toggle(pm, pad + 200.0, y + 1.0, val);
        panel.zones.push(Zone { x0: pad, y0: y, x1: pad + 240.0, y1: y + 22.0, action });
        y += 28.0;
    }
    y += 8.0;

    section_header(pm, font, "Colors", pad, y); y += 22.0;

    let color_fields: &[(&str, LaunchField)] = &[
        ("Background",      LaunchField::Background),
        ("Panel background", LaunchField::PanelBg),
        ("Text color",      LaunchField::TextColor),
        ("Text dim",        LaunchField::TextDim),
        ("Accent",          LaunchField::AccentColor),
        ("Selection",       LaunchField::SelectionColor),
        ("Border",          LaunchField::BorderColor),
    ];

    let mut col_i = 0usize;
    for (label, field) in color_fields {
        let val: String = match field {
            LaunchField::Background    => panel.launch_inputs.background.value.clone(),
            LaunchField::PanelBg       => panel.launch_inputs.panel_bg.value.clone(),
            LaunchField::TextColor     => panel.launch_inputs.text_color.value.clone(),
            LaunchField::TextDim       => panel.launch_inputs.text_dim.value.clone(),
            LaunchField::AccentColor   => panel.launch_inputs.accent_color.value.clone(),
            LaunchField::SelectionColor => panel.launch_inputs.selection.value.clone(),
            LaunchField::BorderColor   => panel.launch_inputs.border_color.value.clone(),
            _ => String::new(),
        };
        let field = field.clone();
        let fx = pad + col_i as f32 * (fw3 + pad);
        if val.starts_with('#') { fill_rrect(pm, fx, y, 12.0, 12.0, 3.0, &val); }
        let f = panel.launch_focused == Some(field.clone());
        draw_field(pm, font, label, &val, f, fx, y, fw3, 40.0);
        panel.zones.push(Zone { x0: fx, y0: y, x1: fx + fw3, y1: y + 40.0,
                                action: ZoneAction::LaunchFieldFocus(field) });
        col_i += 1;
        if col_i == 3 { col_i = 0; y += 48.0; }
    }
}

fn section_header(pm: &mut Pixmap, font: &fontdue::Font, label: &str, x: f32, y: f32) {
    draw_text(pm, font, label, x, y, 11.0, ACCENT);
    fill_rect(pm, x + measure(font, label, 11.0) + 8.0, y + 6.0, 400.0, 1.0, BORDER);
}

const ACCENT: &str = "#c792ea";
const FG:     &str = "#cdd6f4";
const BORDER: &str = "#2a1545";
