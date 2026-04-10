//! Main bar render loop.
//! Layout: [left widgets] | [center widget(s) — self-position] | [right widgets]
//! Polls Sway state and redraws on change or every 1s for time/sys updates.

use anyhow::Result;
use crossbeam_channel::unbounded;
use tiny_skia::{Color, Pixmap};
use tokio::runtime::Runtime;

use crate::config::{BarConfig, ModuleKind};
use crate::draw::{clear, fill_rect, hex_color};
use crate::icons::IconCache;
use crate::sway::{BarState as SwayState, SwayClient};
use crate::text::TextRenderer;
use crate::wayland::{BarSurface, MouseEvent};
use crate::widgets::{
    RenderCtx, Widget,
    activities::ActivitiesWidget,
    audio::AudioWidget,
    battery::BatteryWidget,
    clock::ClockWidget,
    control_center::ControlCenterWidget,
    cpu::CpuWidget,
    disk::DiskWidget,
    media::MediaWidget,
    memory::MemoryWidget,
    network::NetworkWidget,
    notifications::NotificationsWidget,
    systray::SystrayWidget,
    temp::TempWidget,
    window_title::WindowTitleWidget,
    workspaces::WorkspacesWidget,
};

// ── Click zone tracking ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Zone { Left, Center, Right }

#[derive(Clone)]
struct ClickZone {
    x0:   f32,
    x1:   f32,
    zone: Zone,
    idx:  usize,
}

// ─────────────────────────────────────────────────────────────────────────────

pub fn run(cfg: BarConfig) -> Result<()> {
    let rt = Runtime::new()?;

    let (mouse_tx, mouse_rx) = unbounded::<MouseEvent>();
    let mut surface = BarSurface::new(&cfg.position, cfg.height, mouse_tx)?;

    // Wait for at least one output to be configured
    for _ in 0..50 {
        surface.dispatch()?;
        if surface.output_count() > 0 { break; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if surface.output_count() == 0 {
        anyhow::bail!("no Wayland outputs became available");
    }

    // Build widget lists from config
    let mut left   = build_widgets(&cfg.modules.left);
    let mut center = build_widgets(&cfg.modules.center);
    let mut right  = build_widgets(&cfg.modules.right);

    let mut text       = TextRenderer::new();
    let mut icons      = IconCache::new();
    let mut state      = SwayState::default();
    let mut click_zones: Vec<ClickZone> = Vec::new();

    // Sway event subscription (optional — gracefully absent if not on sway)
    let mut sway_rx = if SwayClient::detect() {
        let client = SwayClient::new()?;
        let rx = {
            let _guard = rt.enter();
            client.subscribe()
        };

        // Initial fetch
        state = rt.block_on(async {
            let mut s = SwayState::default();
            if let Ok(ws) = client.workspaces().await { s.workspaces = ws; }
            if let Ok((title, class)) = client.focused_window().await {
                s.active_title = title;
                s.active_class = class;
            }
            s
        });

        Some((client, rx))
    } else {
        tracing::warn!("SWAYSOCK not set — workspace/title widgets inactive");
        None
    };

    let tick = std::time::Duration::from_millis(1000);
    let mut last_tick = std::time::Instant::now();

    loop {
        surface.dispatch()?;

        // Handle mouse clicks
        while let Ok(ev) = mouse_rx.try_recv() {
            if let MouseEvent::Press { x, y } = ev {
                handle_click(x, y, &click_zones, &mut left, &mut center, &mut right);
            }
        }

        // Poll sway events
        if let Some((ref client, ref mut rx)) = sway_rx {
            let mut dirty = false;
            while rx.try_recv().is_ok() { dirty = true; }
            if dirty {
                state = rt.block_on(async {
                    let mut s = SwayState::default();
                    if let Ok(ws) = client.workspaces().await { s.workspaces = ws; }
                    if let Ok((title, class)) = client.focused_window().await {
                        s.active_title = title;
                        s.active_class = class;
                    }
                    s
                });
                text.clear_dynamic_cache();
            }
        }

        if last_tick.elapsed() >= tick {
            last_tick = std::time::Instant::now();
            let zones = &mut click_zones;
            surface.present_for_each(|w, h| {
                zones.clear();
                render_frame(
                    w, h, &cfg,
                    &mut left, &mut center, &mut right,
                    &mut text, &mut icons, &state,
                    zones,
                )
            })?;
        }

        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn render_frame(
    width: u32,
    height: u32,
    cfg: &BarConfig,
    left:   &mut Vec<Box<dyn Widget>>,
    center: &mut Vec<Box<dyn Widget>>,
    right:  &mut Vec<Box<dyn Widget>>,
    text:   &mut TextRenderer,
    icons:  &mut IconCache,
    state:  &SwayState,
    zones:  &mut Vec<ClickZone>,
) -> Vec<u8> {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    clear(&mut pixmap, hex_color(&cfg.theme.background));

    let mut ctx = RenderCtx {
        pixmap: &mut pixmap,
        text,
        icons,
        theme: &cfg.theme,
        state,
        height,
    };

    // ── Left zone ─────────────────────────────────────────────────────────────
    let mut x = 6.0f32;
    for (i, w) in left.iter_mut().enumerate() {
        let ww = w.width(ctx.theme, ctx.text) as f32;
        let x0 = x;
        w.render(&mut ctx, x);
        zones.push(ClickZone { x0, x1: x + ww, zone: Zone::Left, idx: i });
        x += ww + 4.0;
    }

    // ── Right zone (measure first, render left-to-right) ─────────────────────
    let right_gap    = 10.0f32;
    let outer_pad    = 10.0f32;
    let right_widths: Vec<f32> = right.iter()
        .map(|w| w.width(ctx.theme, ctx.text) as f32)
        .collect();
    let visible: Vec<(usize, f32)> = right_widths.iter()
        .copied()
        .enumerate()
        .filter(|(_, w)| *w > 0.0)
        .collect();
    let total_right: f32 = visible.iter().map(|(_, w)| w).sum::<f32>()
        + right_gap * visible.len().saturating_sub(1) as f32;
    let mut right_x = width as f32 - total_right - outer_pad;
    for (i, ww) in &visible {
        let x0 = right_x;
        right.get_mut(*i).unwrap().render(&mut ctx, right_x);
        zones.push(ClickZone { x0, x1: right_x + ww, zone: Zone::Right, idx: *i });
        right_x += ww + right_gap;
    }

    // ── Center zone (widgets self-position) ───────────────────────────────────
    for (i, w) in center.iter_mut().enumerate() {
        w.render(&mut ctx, x);
        // Compute actual rendered bounds: widget centers itself at bar mid
        let cw = w.width(ctx.theme, ctx.text) as f32;
        let cx = (width as f32 / 2.0 - cw / 2.0).max(x);
        zones.push(ClickZone { x0: cx, x1: cx + cw, zone: Zone::Center, idx: i });
    }

    // Convert tiny-skia premultiplied RGBA → wl_shm ARGB8888
    let data = pixmap.data();
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        out.push(chunk[2]); // B
        out.push(chunk[1]); // G
        out.push(chunk[0]); // R
        out.push(chunk[3]); // A
    }
    out
}

fn handle_click(
    x: f64,
    y: f64,
    zones: &[ClickZone],
    left:   &mut Vec<Box<dyn Widget>>,
    center: &mut Vec<Box<dyn Widget>>,
    right:  &mut Vec<Box<dyn Widget>>,
) {
    let fx = x as f32;
    // Find the innermost matching zone (last pushed = highest layer)
    for zone in zones.iter().rev() {
        if fx >= zone.x0 && fx <= zone.x1 {
            match zone.zone {
                Zone::Left   => { if let Some(w) = left.get_mut(zone.idx)   { w.on_click(x, y); } }
                Zone::Center => { if let Some(w) = center.get_mut(zone.idx) { w.on_click(x, y); } }
                Zone::Right  => { if let Some(w) = right.get_mut(zone.idx)  { w.on_click(x, y); } }
            }
            return;
        }
    }
}

fn build_widgets(kinds: &[ModuleKind]) -> Vec<Box<dyn Widget>> {
    kinds.iter().map(|k| -> Box<dyn Widget> {
        match k {
            ModuleKind::Activities    => Box::new(ActivitiesWidget::new()),
            ModuleKind::Workspaces    => Box::new(WorkspacesWidget::new()),
            ModuleKind::WindowTitle   => Box::new(WindowTitleWidget::new()),
            ModuleKind::Clock         => Box::new(ClockWidget::new()),
            ModuleKind::Battery       => Box::new(BatteryWidget::new()),
            ModuleKind::Audio         => Box::new(AudioWidget::new()),
            ModuleKind::Network       => Box::new(NetworkWidget::new()),
            ModuleKind::Systray       => Box::new(SystrayWidget::new()),
            ModuleKind::Cpu           => Box::new(CpuWidget::new()),
            ModuleKind::Memory        => Box::new(MemoryWidget::new()),
            ModuleKind::Disk          => Box::new(DiskWidget::new("/")),
            ModuleKind::Temp          => Box::new(TempWidget::new()),
            ModuleKind::Media         => Box::new(MediaWidget::new()),
            ModuleKind::Notifications => Box::new(NotificationsWidget::new()),
            ModuleKind::ControlCenter => Box::new(ControlCenterWidget::new()),
        }
    }).collect()
}
