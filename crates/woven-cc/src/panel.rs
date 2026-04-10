//! Control center panel — state + tiny-skia rendering.

use tiny_skia::*;

// ── Dimensions ────────────────────────────────────────────────────────────────
pub const WIDTH: u32 = 360;

// Colors (TWM palette)
const BG:      &str = "#0d0018";
const BG_CARD: &str = "#160026";
const ACCENT:  &str = "#c792ea";
const TEAL:    &str = "#00e5c8";
const FG:      &str = "#cdd6f4";
const DIM:     &str = "#4a3060";
const RED:     &str = "#f07178";
const BORDER:  &str = "#2a1545";

// ── Click zone registry ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Btn {
    DndToggle,
    Lock, Suspend, Logout, Reboot, Shutdown,
    BrightnessDown, BrightnessUp,
    VolumeDown, VolumeUp,
    MuteToggle,
}

#[derive(Clone)]
pub struct Zone {
    pub btn: Btn,
    pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32,
}

// ── Panel state ───────────────────────────────────────────────────────────────

pub struct Panel {
    pub dnd:        bool,
    pub brightness: u8,
    pub volume:     u8,
    pub muted:      bool,
    pub username:   String,
    pub hostname:   String,
    pub pfp:        Option<Vec<u8>>, // RGBA, 64×64
    pub zones:      Vec<Zone>,
    text_font:      fontdue::Font,
    icon_font:      Option<fontdue::Font>,
}

impl Panel {
    pub fn new() -> Self {
        let text_font = load_font_from_paths(&[
            "/usr/share/fonts/TTF/Inconsolata-Regular.ttf",
            "/usr/share/fonts/TTF/Inconsolata.ttf",
            "/usr/share/fonts/truetype/inconsolata/Inconsolata-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
        ]).expect("no usable text font found");

        let icon_font = load_font_from_paths(&[
            "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMono Nerd Font Regular.ttf",
            "/usr/share/fonts/OTF/JetBrainsMonoNerdFont-Regular.otf",
            "/usr/share/fonts/TTF/FiraCodeNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/HackNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/SymbolsNerdFont-Regular.ttf",
            "/usr/share/fonts/TTF/SymbolsNerdFontMono-Regular.ttf",
        ]);

        let pfp      = load_pfp();
        let dnd      = read_dnd();
        let (volume, muted) = read_volume();
        let brightness = read_brightness();
        let username = std::env::var("USER").unwrap_or_else(|_| "abyss".into());
        let hostname = std::fs::read_to_string("/etc/hostname")
            .unwrap_or_default().trim().to_string();

        Self { dnd, brightness, volume, muted, username, hostname, pfp,
               zones: vec![], text_font, icon_font }
    }

    pub fn refresh_states(&mut self) {
        self.dnd = read_dnd();
        let (v, m) = read_volume();
        self.volume = v; self.muted = m;
        self.brightness = read_brightness();
    }

    /// Render the panel. Returns BGRA pixel bytes for wl_shm and the computed height.
    pub fn render(&mut self) -> (Vec<u8>, u32) {
        self.zones.clear();

        let font_size = 13.5f32;
        let small     = 11.5f32;
        let w         = WIDTH as f32;
        let pad       = 14.0f32;

        let header_h  = 76.0f32;
        let row_h     = 42.0f32;
        let sep_h     = 10.0f32;
        let power_h   = 68.0f32; // single row of icon+label buttons

        let height = (pad + header_h + sep_h
                      + row_h              // DND
                      + sep_h
                      + power_h           // power buttons
                      + sep_h
                      + row_h             // brightness
                      + row_h             // volume
                      + pad) as u32;

        let mut pm = Pixmap::new(WIDTH, height).unwrap();
        pm.fill(hex(BG));

        // Outer border
        draw_rrect(&mut pm, 0.5, 0.5, w - 1.0, height as f32 - 1.0, 0.0, hex(BORDER));

        let mut y = pad;

        // ── Header ────────────────────────────────────────────────────────────
        {
            let pfp_sz = 48.0f32;
            let pfp_x  = pad;
            let pfp_y  = y + (header_h - pfp_sz) / 2.0;

            // pfp circle
            draw_circle(&mut pm, pfp_x + pfp_sz / 2.0, pfp_y + pfp_sz / 2.0,
                        pfp_sz / 2.0 + 2.0, hex(BORDER));

            if let Some(ref rgba) = self.pfp {
                blit_circle(&mut pm, rgba, 64, pfp_x as u32, pfp_y as u32, pfp_sz as u32);
            } else {
                draw_circle(&mut pm, pfp_x + pfp_sz / 2.0, pfp_y + pfp_sz / 2.0,
                            pfp_sz / 2.0, hex(DIM));
                self.draw_icon(&mut pm, "\u{f0004}", pfp_x + 10.0, pfp_y + 12.0, 24.0, hex(FG));
            }

            let tx = pfp_x + pfp_sz + 12.0;
            self.draw_text(&mut pm, &self.username.clone(), tx, y + 20.0, font_size + 2.0, hex(FG));
            self.draw_text(&mut pm, &self.hostname.clone(), tx, y + 38.0, small, hex(DIM));
        }
        y += header_h;

        // ── Separator ─────────────────────────────────────────────────────────
        draw_rect(&mut pm, pad, y + 2.0, w - pad * 2.0, 1.0, hex(BORDER));
        y += sep_h;

        // ── DND row ───────────────────────────────────────────────────────────
        {
            let ry = y;
            draw_rrect(&mut pm, pad, ry, w - pad * 2.0, row_h - 4.0, 6.0, hex(BG_CARD));

            let (icon, fg_c) = if self.dnd {
                ("\u{f09a7}", hex(RED))
            } else {
                ("\u{f09a4}", hex(FG))
            };
            self.draw_icon(&mut pm, icon, pad + 10.0, ry + 12.0, font_size + 2.0, fg_c);
            self.draw_text(&mut pm, "Do Not Disturb", pad + 36.0, ry + 14.0, font_size, hex(FG));

            let state_txt = if self.dnd { "ON " } else { "OFF" };
            let st_col    = if self.dnd { hex(RED) } else { hex(DIM) };
            let st_w = measure_text(&self.text_font, state_txt, small) + 10.0;
            draw_rrect(&mut pm, w - pad - st_w, ry + 9.0, st_w, row_h - 22.0, 5.0, st_col);
            self.draw_text(&mut pm, state_txt, w - pad - st_w + 5.0, ry + 14.0, small, hex(BG));

            self.zones.push(Zone { btn: Btn::DndToggle,
                x0: pad, y0: ry, x1: w - pad, y1: ry + row_h - 4.0 });
            y += row_h + 4.0;
        }

        // ── Separator ─────────────────────────────────────────────────────────
        draw_rect(&mut pm, pad, y + 2.0, w - pad * 2.0, 1.0, hex(BORDER));
        y += sep_h;

        // ── Power buttons ─────────────────────────────────────────────────────
        let power_btns: &[(Btn, &str, &str, &str)] = &[
            (Btn::Lock,     "\u{f033e}", "Lock",     FG),
            (Btn::Suspend,  "\u{f04b2}", "Suspend",  FG),
            (Btn::Logout,   "\u{f0343}", "Logout",   FG),
            (Btn::Reboot,   "\u{f0709}", "Reboot",   FG),
            (Btn::Shutdown, "\u{f0425}", "Shutdown", RED),
        ];

        let btn_w = (w - pad * 2.0) / power_btns.len() as f32;
        for (i, &(btn, icon, label, col)) in power_btns.iter().enumerate() {
            let bx = pad + i as f32 * btn_w;
            draw_rrect(&mut pm, bx + 2.0, y + 2.0, btn_w - 4.0, power_h - 4.0, 8.0, hex(BG_CARD));

            let ic_w = measure_icon(&self.icon_font, &self.text_font, icon, font_size + 2.0);
            self.draw_icon(&mut pm, icon, bx + (btn_w - ic_w) / 2.0, y + 10.0, font_size + 4.0, hex(col));

            let lw = measure_text(&self.text_font, label, small - 0.5);
            self.draw_text(&mut pm, label, bx + (btn_w - lw) / 2.0,
                           y + power_h - small - 12.0, small - 0.5, hex(col));

            self.zones.push(Zone { btn,
                x0: bx + 2.0, y0: y + 2.0, x1: bx + btn_w - 2.0, y1: y + power_h - 2.0 });
        }
        y += power_h + 4.0;

        // ── Separator ─────────────────────────────────────────────────────────
        draw_rect(&mut pm, pad, y + 2.0, w - pad * 2.0, 1.0, hex(BORDER));
        y += sep_h;

        // ── Slider rows ───────────────────────────────────────────────────────
        let sliders: &[(Btn, Btn, &str, &str, u8, bool)] = &[
            (Btn::BrightnessDown, Btn::BrightnessUp,
             "\u{f00e3}", "Brightness", self.brightness, false),
            (Btn::VolumeDown, Btn::VolumeUp,
             "\u{f057f}", "Volume", self.volume, self.muted),
        ];

        for &(btn_dn, btn_up, icon, label, val, is_muted) in sliders {
            let ry = y;
            draw_rrect(&mut pm, pad, ry, w - pad * 2.0, row_h - 4.0, 6.0, hex(BG_CARD));

            let dim_fg = if is_muted { hex(DIM) } else { hex(FG) };
            self.draw_icon(&mut pm, icon, pad + 10.0, ry + 12.0, font_size + 2.0, dim_fg);
            self.draw_text(&mut pm, label, pad + 34.0, ry + 14.0, font_size, dim_fg);

            let val_str = if is_muted { "MUTED".to_string() } else { format!("{val}%") };
            let vw = measure_text(&self.text_font, &val_str, small);
            self.draw_text(&mut pm, &val_str, w / 2.0 - vw / 2.0, ry + 14.0, small, hex(ACCENT));

            let btn_sz = 26.0f32;
            let mx = w - pad - btn_sz * 2.0 - 8.0;
            let px = w - pad - btn_sz;
            let by = ry + (row_h - 4.0 - btn_sz) / 2.0;

            draw_rrect(&mut pm, mx, by, btn_sz, btn_sz, 6.0, hex(BORDER));
            let mw = measure_text(&self.text_font, "−", font_size);
            self.draw_text(&mut pm, "−", mx + (btn_sz - mw) / 2.0,
                           by + (btn_sz - font_size) / 2.0, font_size, hex(FG));

            draw_rrect(&mut pm, px, by, btn_sz, btn_sz, 6.0, hex(BORDER));
            let pw_m = measure_text(&self.text_font, "+", font_size);
            self.draw_text(&mut pm, "+", px + (btn_sz - pw_m) / 2.0,
                           by + (btn_sz - font_size) / 2.0, font_size, hex(FG));

            self.zones.push(Zone { btn: btn_dn, x0: mx, y0: ry, x1: mx + btn_sz, y1: ry + row_h });
            self.zones.push(Zone { btn: btn_up, x0: px, y0: ry, x1: px + btn_sz, y1: ry + row_h });
            if btn_dn == Btn::VolumeDown {
                self.zones.push(Zone { btn: Btn::MuteToggle,
                    x0: pad, y0: ry, x1: w / 2.0 - 20.0, y1: ry + row_h });
            }

            y += row_h + 4.0;
        }

        // tiny-skia premultiplied RGBA → wl_shm ARGB8888 (swap R/B)
        let data = pm.data();
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(4) {
            out.push(chunk[2]); // B
            out.push(chunk[1]); // G
            out.push(chunk[0]); // R
            out.push(chunk[3]); // A
        }
        (out, height)
    }

    pub fn handle(&mut self, btn: Btn) -> bool {
        match btn {
            Btn::DndToggle => {
                let _ = std::process::Command::new("swaync-client").arg("-d").spawn();
                self.dnd = !self.dnd;
            }
            Btn::Lock     => { let _ = std::process::Command::new("swaylock").arg("-f").spawn(); return true; }
            Btn::Suspend  => { let _ = std::process::Command::new("systemctl").arg("suspend").spawn(); return true; }
            Btn::Logout   => { let _ = std::process::Command::new("swaymsg").arg("exit").spawn(); return true; }
            Btn::Reboot   => { let _ = std::process::Command::new("systemctl").arg("reboot").spawn(); return true; }
            Btn::Shutdown => { let _ = std::process::Command::new("systemctl").arg("poweroff").spawn(); return true; }
            Btn::BrightnessDown => {
                let _ = std::process::Command::new("brightnessctl").args(["set", "10%-"]).spawn();
                std::thread::sleep(std::time::Duration::from_millis(200));
                self.brightness = read_brightness();
            }
            Btn::BrightnessUp => {
                let _ = std::process::Command::new("brightnessctl").args(["set", "10%+"]).spawn();
                std::thread::sleep(std::time::Duration::from_millis(200));
                self.brightness = read_brightness();
            }
            Btn::VolumeDown => {
                let _ = std::process::Command::new("wpctl")
                    .args(["set-volume", "@DEFAULT_AUDIO_SINK@", "10%-"]).spawn();
                std::thread::sleep(std::time::Duration::from_millis(100));
                let (v, m) = read_volume(); self.volume = v; self.muted = m;
            }
            Btn::VolumeUp => {
                let _ = std::process::Command::new("wpctl")
                    .args(["set-volume", "@DEFAULT_AUDIO_SINK@", "10%+"]).spawn();
                std::thread::sleep(std::time::Duration::from_millis(100));
                let (v, m) = read_volume(); self.volume = v; self.muted = m;
            }
            Btn::MuteToggle => {
                let _ = std::process::Command::new("wpctl")
                    .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).spawn();
                std::thread::sleep(std::time::Duration::from_millis(100));
                let (v, m) = read_volume(); self.volume = v; self.muted = m;
            }
        }
        false
    }

    // ── Text rendering ────────────────────────────────────────────────────────

    fn draw_text(&self, pm: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: Color) {
        blit_text(pm, &self.text_font, text, x, y, size, color);
    }

    fn draw_icon(&self, pm: &mut Pixmap, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let font = self.icon_font.as_ref().unwrap_or(&self.text_font);
        blit_text(pm, font, text, x, y, size, color);
    }
}

// ── System reads ──────────────────────────────────────────────────────────────

fn read_dnd() -> bool {
    std::process::Command::new("swaync-client").arg("-D").output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "true").unwrap_or(false)
}

fn read_volume() -> (u8, bool) {
    let out = std::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output().ok();
    let Some(o) = out else { return (0, false); };
    let s = String::from_utf8_lossy(&o.stdout);
    let muted = s.contains("[MUTED]");
    let vol: f32 = s.split_whitespace()
        .find(|w| w.parse::<f32>().is_ok())
        .and_then(|w| w.parse().ok()).unwrap_or(0.0);
    ((vol * 100.0).min(100.0) as u8, muted)
}

fn read_brightness() -> u8 {
    // Try brightnessctl -m for machine-readable output: name,full,type,cur,max,pct%
    if let Ok(o) = std::process::Command::new("brightnessctl").args(["-m", "g"]).output() {
        let s = String::from_utf8_lossy(&o.stdout);
        if let Some(pct) = s.trim().split(',').nth(5) {
            if let Ok(v) = pct.trim_end_matches('%').parse::<u8>() {
                return v;
            }
        }
    }
    // Sysfs fallback
    if let Ok(entries) = std::fs::read_dir("/sys/class/backlight") {
        for entry in entries.flatten() {
            let base = entry.path();
            let cur: u64 = std::fs::read_to_string(base.join("brightness"))
                .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let max: u64 = std::fs::read_to_string(base.join("max_brightness"))
                .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1);
            if max > 0 { return ((cur * 100 / max) as u8).min(100); }
        }
    }
    0
}

fn load_pfp() -> Option<Vec<u8>> {
    for p in &["/var/lib/AccountsService/icons/abyss", "/home/abyss/.face"] {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(img) = image::load_from_memory(&data) {
                let r = img.resize_exact(64, 64, image::imageops::FilterType::Lanczos3);
                return Some(r.to_rgba8().into_raw());
            }
        }
    }
    None
}

fn load_font_from_paths(paths: &[&str]) -> Option<fontdue::Font> {
    for p in paths {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(f) = fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()) {
                return Some(f);
            }
        }
    }
    None
}

// ── Drawing primitives ────────────────────────────────────────────────────────

fn hex(s: &str) -> Color {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0xFFFFFF);
    Color::from_rgba8((v >> 16) as u8, (v >> 8 & 0xFF) as u8, (v & 0xFF) as u8, 255)
}

fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}

fn draw_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    let Some(rect) = Rect::from_xywh(x, y, w, h) else { return };
    let mut pa = paint(color);
    pa.anti_alias = false;
    pm.fill_rect(rect, &pa, Transform::identity(), None);
}

fn draw_rrect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    if r <= 0.0 || w < r * 2.0 || h < r * 2.0 { draw_rect(pm, x, y, w, h, color); return; }
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
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color) {
    if r <= 0.0 { return; }
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
    }
}

fn measure_text(font: &fontdue::Font, text: &str, size: f32) -> f32 {
    text.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

fn measure_icon(icon_font: &Option<fontdue::Font>, text_font: &fontdue::Font, text: &str, size: f32) -> f32 {
    let font = icon_font.as_ref().unwrap_or(text_font);
    measure_text(font, text, size)
}

/// Rasterize text into the pixmap using the given fontdue font.
fn blit_text(pm: &mut Pixmap, font: &fontdue::Font, text: &str, x: f32, y: f32, size: f32, color: Color) {
    let r = (color.red()   * 255.0) as u8;
    let g = (color.green() * 255.0) as u8;
    let b = (color.blue()  * 255.0) as u8;
    let a_base = (color.alpha() * 255.0) as u8;
    if a_base == 0 { return; }

    let pw = pm.width()  as i32;
    let ph = pm.height() as i32;
    let mut cx = x;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        if metrics.width == 0 { cx += metrics.advance_width; continue; }

        let gx = (cx + metrics.xmin as f32).round() as i32;
        let gy = (y  + size - metrics.height as f32 - metrics.ymin as f32).round() as i32;

        let pixels = pm.pixels_mut();
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + col];
                if coverage == 0 { continue; }
                let px = gx + col as i32;
                let py = gy + row as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph { continue; }

                let idx   = (py * pw + px) as usize;
                let dst   = &mut pixels[idx];
                let src_a = (coverage as u16 * a_base as u16 / 255) as u8;
                let inv_a = 255u16 - src_a as u16;

                // Porter-Duff "over" (destination is already premultiplied)
                let dr = ((r as u16 * src_a as u16 / 255 + dst.red()   as u16 * inv_a / 255)) as u8;
                let dg = ((g as u16 * src_a as u16 / 255 + dst.green() as u16 * inv_a / 255)) as u8;
                let db = ((b as u16 * src_a as u16 / 255 + dst.blue()  as u16 * inv_a / 255)) as u8;
                let da = (src_a as u16 + dst.alpha() as u16 * inv_a / 255).min(255) as u8;

                *dst = PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
            }
        }
        cx += metrics.advance_width;
    }
}

/// Blit a 64×64 RGBA (non-premultiplied) image as a clipped circle into the pixmap.
fn blit_circle(pm: &mut Pixmap, rgba: &[u8], src_sz: u32, dst_x: u32, dst_y: u32, size: u32) {
    let cx    = size as f32 / 2.0;
    let cy    = size as f32 / 2.0;
    let r     = cx;
    let scale = src_sz as f32 / size as f32;
    let pw    = pm.width()  as i32;
    let ph    = pm.height() as i32;

    let pixels = pm.pixels_mut();
    for py in 0..size {
        for px in 0..size {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            if dx * dx + dy * dy > r * r { continue; }

            let sx = ((px as f32 * scale) as u32).min(src_sz - 1) as usize;
            let sy = ((py as f32 * scale) as u32).min(src_sz - 1) as usize;
            let si = (sy * src_sz as usize + sx) * 4;
            if si + 3 >= rgba.len() { continue; }

            let [sr, sg, sb, sa] = [rgba[si], rgba[si+1], rgba[si+2], rgba[si+3]];
            if sa == 0 { continue; }

            let ox = dst_x + px;
            let oy = dst_y + py;
            if ox as i32 >= pw || oy as i32 >= ph { continue; }

            let idx = (oy as i32 * pw + ox as i32) as usize;
            // Convert non-premultiplied RGBA → premultiplied (tiny-skia format [R,G,B,A] pm)
            let sa16 = sa as u16;
            let pr = (sr as u16 * sa16 / 255) as u8;
            let pg = (sg as u16 * sa16 / 255) as u8;
            let pb_c = (sb as u16 * sa16 / 255) as u8;
            pixels[idx] = PremultipliedColorU8::from_rgba(pr, pg, pb_c, sa).unwrap_or(pixels[idx]);
        }
    }
}
