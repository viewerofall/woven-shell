//! Sway keybind editor tab.

use tiny_skia::Pixmap;
use crate::panel::*;

const ROW:   f32 = 36.0;
const PAD:   f32 = 14.0;
const COL_KEY:  f32 = 170.0;
const COL_LBL:  f32 = 160.0;
const BTN_W:    f32 = 34.0;
const EDIT_H:   f32 = 120.0;

pub fn render(panel: &mut Panel, pm: &mut Pixmap, w: f32) {
    let font = &panel.text_font.clone();
    let mut y = 8.0f32;

    // "Add bind" button
    let add_x = w - BTN_W * 2.0 - PAD - 4.0;
    fill_rrect(pm, add_x, y, 80.0, 26.0, 6.0, ACCENT_CONST);
    draw_text(pm, font, "+ Add bind", add_x + 8.0, y + 7.0, 12.0, BG_CONST);

    y += 38.0;

    let binds = panel.cfg.keybinds.binds.clone();
    let mut last_cat = "";

    for (i, bind) in binds.iter().enumerate() {
        // Category header
        if bind.category != last_cat {
            last_cat = &bind.category;
            let hdr = category_header(bind.category.as_str());
            fill_rect(pm, PAD, y, w - PAD * 2.0, 1.0, BORDER_CONST);
            draw_text(pm, font, hdr, PAD, y + 4.0, 10.5, DIM_CONST);
            y += 20.0;
        }

        // If this is the bind being edited, render editor instead
        if let Some(ref edit) = panel.sway_edit.clone() {
            if edit.idx == Some(i) {
                render_editor(panel, pm, font, w, y, edit);
                y += EDIT_H + 6.0;
                continue;
            }
        }

        // Bind row
        let row_bg = if panel.sway_hover == Some(i) { BG_HOVER_CONST } else { BG_CARD_CONST };
        fill_rrect(pm, PAD, y, w - PAD * 2.0, ROW - 4.0, 6.0, row_bg);

        // Category pill
        let cat_col = cat_color(&bind.category);
        let pill_w = draw_pill(pm, font, &bind.category, PAD + 6.0, y + 6.0, 22.0, BG_CONST, cat_col);
        let _ = pill_w;

        // Key
        let kx = PAD + 90.0;
        draw_text(pm, font, &bind.key, kx, y + 10.0, 12.0, ACCENT_CONST);

        // Label
        let lx = kx + COL_KEY;
        draw_text(pm, font, &bind.label, lx, y + 10.0, 12.0, FG_CONST);

        // Action (truncated)
        let ax = lx + COL_LBL;
        let avail = w - ax - PAD - BTN_W * 2.0 - 8.0;
        let action_trunc = truncate(font, &bind.action, 11.0, avail);
        draw_text(pm, font, &action_trunc, ax, y + 11.0, 11.0, DIM_CONST);

        // Edit button
        let ex = w - PAD - BTN_W * 2.0 - 4.0;
        fill_rrect(pm, ex, y + 4.0, BTN_W, 26.0, 5.0, BORDER_CONST);
        draw_text(pm, font, "✎", ex + 10.0, y + 8.0, 12.0, FG_CONST);

        // Delete button
        let dx = w - PAD - BTN_W;
        fill_rrect(pm, dx, y + 4.0, BTN_W, 26.0, 5.0, BORDER_CONST);
        draw_text(pm, font, "✕", dx + 10.0, y + 8.0, 12.0, RED_CONST);

        // Register zones (already in inner pixmap coords; caller offsets)
        panel.zones.push(Zone { x0: ex, y0: y + 4.0, x1: ex + BTN_W, y1: y + 30.0,
                                action: ZoneAction::SwayEdit(i) });
        panel.zones.push(Zone { x0: dx, y0: y + 4.0, x1: dx + BTN_W, y1: y + 30.0,
                                action: ZoneAction::SwayDelete(i) });

        y += ROW;
    }

    // If adding a new bind (no existing idx), render editor at bottom
    if let Some(ref edit) = panel.sway_edit.clone() {
        if edit.idx.is_none() {
            fill_rect(pm, PAD, y, w - PAD * 2.0, 1.0, BORDER_CONST);
            y += 8.0;
            draw_text(pm, font, "New bind", PAD, y, 10.5, DIM_CONST);
            y += 18.0;
            render_editor(panel, pm, font, w, y, edit);
        }
    }

    // Register "Add bind" zone at top (after rendering so it's on top)
    panel.zones.push(Zone { x0: add_x, y0: 8.0, x1: add_x + 80.0, y1: 34.0,
                            action: ZoneAction::SwayAdd });
}

fn render_editor(panel: &mut Panel, pm: &mut Pixmap, font: &fontdue::Font,
                 w: f32, y: f32, edit: &SwayEditState) {
    // Editor card
    fill_rrect(pm, PAD, y, w - PAD * 2.0, EDIT_H, 8.0, BG_CARD_CONST);
    stroke_rrect(pm, PAD, y, w - PAD * 2.0, EDIT_H, 8.0, ACCENT_CONST);

    let iw = (w - PAD * 2.0 - 32.0) / 3.0;

    // Key field
    let key_focused = edit.focused == SwayField::Key;
    let key_val = if edit.key.capturing {
        "Press key combo...".to_string()
    } else {
        edit.key.value.clone()
    };
    let key_col = if edit.key.capturing { "teal" } else if key_focused { ACCENT_CONST } else { BORDER_CONST };
    draw_field_inner(pm, font, "Key combo", &key_val, edit.key.capturing || key_focused,
                     key_col, PAD + 12.0, y + 12.0, iw, 40.0);

    // Label field
    let lbl_focused = edit.focused == SwayField::Label;
    draw_field_inner(pm, font, "Label", &edit.label.value, lbl_focused,
                     if lbl_focused { ACCENT_CONST } else { BORDER_CONST },
                     PAD + 12.0 + iw + 8.0, y + 12.0, iw, 40.0);

    // Action field
    let act_focused = edit.focused == SwayField::Action;
    draw_field_inner(pm, font, "Action (sway command)", &edit.action.value, act_focused,
                     if act_focused { ACCENT_CONST } else { BORDER_CONST },
                     PAD + 12.0 + (iw + 8.0) * 2.0, y + 12.0, iw, 40.0);

    // Tab hint
    draw_text(pm, font, "Tab: cycle fields  |  Enter: save  |  Esc: cancel",
              PAD + 12.0, y + 62.0, 10.5, DIM_CONST);

    // Save button
    let sv_x = w - PAD - 80.0 - 90.0;
    fill_rrect(pm, sv_x, y + EDIT_H - 40.0, 80.0, 28.0, 6.0, ACCENT_CONST);
    let sw = measure(font, "Save", 13.0);
    draw_text(pm, font, "Save", sv_x + (80.0 - sw) / 2.0, y + EDIT_H - 32.0, 13.0, BG_CONST);

    // Cancel button
    let cx = w - PAD - 80.0;
    fill_rrect(pm, cx, y + EDIT_H - 40.0, 80.0, 28.0, 6.0, BORDER_CONST);
    let cw = measure(font, "Cancel", 13.0);
    draw_text(pm, font, "Cancel", cx + (80.0 - cw) / 2.0, y + EDIT_H - 32.0, 13.0, FG_CONST);

    // Zone registrations (inner coords)
    let key_x1 = PAD + 12.0 + iw;
    panel.zones.push(Zone { x0: PAD + 12.0, y0: y + 12.0, x1: key_x1, y1: y + 52.0,
                            action: ZoneAction::SwayEditField(SwayField::Key) });
    let lbl_x0 = PAD + 12.0 + iw + 8.0;
    panel.zones.push(Zone { x0: lbl_x0, y0: y + 12.0, x1: lbl_x0 + iw, y1: y + 52.0,
                            action: ZoneAction::SwayEditField(SwayField::Label) });
    let act_x0 = PAD + 12.0 + (iw + 8.0) * 2.0;
    panel.zones.push(Zone { x0: act_x0, y0: y + 12.0, x1: act_x0 + iw, y1: y + 52.0,
                            action: ZoneAction::SwayEditField(SwayField::Action) });
    panel.zones.push(Zone { x0: sv_x, y0: y + EDIT_H - 40.0, x1: sv_x + 80.0, y1: y + EDIT_H - 12.0,
                            action: ZoneAction::SwayEditSave });
    panel.zones.push(Zone { x0: cx, y0: y + EDIT_H - 40.0, x1: cx + 80.0, y1: y + EDIT_H - 12.0,
                            action: ZoneAction::SwayEditCancel });
}

fn draw_field_inner(pm: &mut Pixmap, font: &fontdue::Font, label: &str, val: &str,
                    focused: bool, border_col: &str, x: f32, y: f32, w: f32, h: f32) {
    let bg = if focused { "#1e0038" } else { BG_CONST };
    fill_rrect(pm, x, y, w, h, 6.0, bg);
    stroke_rrect(pm, x, y, w, h, 6.0, border_col);
    draw_text(pm, font, label, x + 6.0, y + 4.0, 9.5, DIM_CONST);
    let avail = w - 12.0;
    let display = truncate(font, val, 11.5, avail);
    draw_text(pm, font, &display, x + 6.0, y + 16.0, 11.5, FG_CONST);
    if focused {
        let cx = x + 6.0 + measure(font, &display, 11.5);
        fill_rect(pm, cx.min(x + w - 4.0), y + 16.0, 1.5, 13.0, ACCENT_CONST);
    }
}

fn category_header(cat: &str) -> &'static str {
    match cat {
        "core"        => "── Core",
        "apps"        => "── Apps",
        "woven"       => "── Woven Shell",
        "focus"       => "── Focus",
        "move"        => "── Move",
        "layout"      => "── Layout",
        "workspaces"  => "── Workspaces",
        "media"       => "── Media / System",
        "screenshots" => "── Screenshots",
        _             => "── Other",
    }
}

fn truncate(font: &fontdue::Font, s: &str, size: f32, max_w: f32) -> String {
    let mut out = String::new();
    let mut width = 0.0f32;
    let ellipsis_w = measure(font, "…", size);
    for ch in s.chars() {
        let cw = font.metrics(ch, size).advance_width;
        if width + cw + ellipsis_w > max_w && !out.is_empty() {
            out.push('…');
            return out;
        }
        out.push(ch);
        width += cw;
    }
    out
}

// ── Const re-exports for use in this module ───────────────────────────────────
// (panel consts are not pub, re-declare locally)
const BG_CONST:      &str = "#0a0010";
const BG_CARD_CONST: &str = "#160026";
const BG_HOVER_CONST:&str = "#1e002e";
const ACCENT_CONST:  &str = "#c792ea";
const FG_CONST:      &str = "#cdd6f4";
const DIM_CONST:     &str = "#6a508a";
const BORDER_CONST:  &str = "#2a1545";
const RED_CONST:     &str = "#f07178";
