//! Wayland background layer-shell surface for woven-wall.
//! Layer::Background — rendered below all windows and bars.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler,
            LayerSurface as SctLayerSurface, LayerSurfaceConfigure,
        },
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_region, wl_shm, wl_surface},
    Connection, EventQueue, Proxy, QueueHandle,
};

struct PerOutput {
    output_id:     u32,
    layer_surface: SctLayerSurface,
    pool:          SlotPool,
    width:         u32,
    height:        u32,
    configured:    bool,
}

pub struct WallSurface {
    queue: EventQueue<WallState>,
    state: WallState,
}

impl WallSurface {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env()
            .context("wall: failed to connect to Wayland display")?;
        let (globals, queue) = registry_queue_init::<WallState>(&conn)
            .context("wall: failed to init Wayland registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("wall: wl_compositor missing")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("wall: wlr-layer-shell missing")?;
        let shm         = Shm::bind(&globals, &qh).context("wall: wl_shm missing")?;

        let state = WallState {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            shm,
            layer_shell,
            outputs: Vec::new(),
        };

        let mut s = Self { queue, state };
        let _ = s.queue.roundtrip(&mut s.state);
        Ok(s)
    }

    pub fn output_count(&self) -> usize {
        self.state.outputs.iter().filter(|o| o.configured).count()
    }

    pub fn first_output_size(&self) -> Option<(u32, u32)> {
        self.state.outputs.iter()
            .find(|o| o.configured && o.width > 0 && o.height > 0)
            .map(|o| (o.width, o.height))
    }

    pub fn dispatch(&mut self) -> Result<()> {
        if let Err(e) = self.queue.flush() {
            tracing::debug!("wall flush: {e}");
        }
        if let Some(guard) = self.queue.prepare_read() {
            use std::os::unix::io::AsRawFd;
            use rustix::fd::AsFd;
            use rustix::event::{PollFd, PollFlags, poll};
            use rustix::time::Timespec;
            let raw      = self.queue.as_fd().as_raw_fd();
            let borrowed = unsafe { rustix::fd::BorrowedFd::borrow_raw(raw) };
            let mut pfd  = PollFd::new(&borrowed, PollFlags::IN);
            let ts       = Timespec { tv_sec: 0, tv_nsec: 0 };
            let ready    = poll(std::slice::from_mut(&mut pfd), Some(&ts)).unwrap_or(0);
            if ready > 0 { let _ = guard.read(); } else { drop(guard); }
        }
        self.queue.dispatch_pending(&mut self.state).context("wall dispatch failed")?;
        if self.state.outputs.iter().any(|o| !o.configured) {
            let _ = self.queue.roundtrip(&mut self.state);
        }
        Ok(())
    }

    /// Call `paint_fn(width, height)` for every configured output and commit
    /// the returned BGRA pixel buffer to that output's background surface.
    pub fn present_for_each<F: FnMut(u32, u32) -> Vec<u8>>(
        &mut self,
        mut paint_fn: F,
    ) -> Result<()> {
        for out in &mut self.state.outputs {
            if out.width == 0 || out.height == 0 { continue; }
            let pixels = paint_fn(out.width, out.height);
            let stride = out.width * 4;
            let (buffer, canvas) = out.pool
                .create_buffer(
                    out.width as i32, out.height as i32,
                    stride as i32, wl_shm::Format::Argb8888,
                )
                .context("wall: create_buffer failed")?;
            let n = canvas.len().min(pixels.len());
            canvas[..n].copy_from_slice(&pixels[..n]);
            let surf = out.layer_surface.wl_surface();
            buffer.attach_to(surf).ok();
            surf.damage_buffer(0, 0, out.width as i32, out.height as i32);
            out.layer_surface.commit();
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────

struct WallState {
    registry:     RegistryState,
    compositor:   CompositorState,
    output_state: OutputState,
    shm:          Shm,
    layer_shell:  LayerShell,
    outputs:      Vec<PerOutput>,
}

impl WallState {
    fn add_output(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        let output_id = output.id().protocol_id();
        if self.outputs.iter().any(|o| o.output_id == output_id) { return; }

        let surface       = self.compositor.create_surface(qh);
        let layer_surface = self.layer_shell.create_layer_surface(
            qh, surface, Layer::Background,
            Some("woven-wall"), Some(output),
        );

        layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.set_size(0, 0);
        layer_surface.commit();

        // 64 MB pool: covers 4K double-buffered (3840×2160×4×2 ≈ 64 MB)
        let pool = match SlotPool::new(64 * 1024 * 1024, &self.shm) {
            Ok(p)  => p,
            Err(e) => { tracing::warn!("wall: shm pool failed for output {output_id}: {e}"); return; }
        };

        tracing::info!("wall: added surface for output {output_id}");
        self.outputs.push(PerOutput {
            output_id,
            layer_surface,
            pool,
            width:      0,
            height:     0,
            configured: false,
        });
    }
}

// ─── Smithay handler impls ────────────────────────────────────────────────────

impl CompositorHandler for WallState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for WallState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.add_output(qh, &output);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let id = output.id().protocol_id();
        self.outputs.retain(|o| o.output_id != id);
    }
}

impl LayerShellHandler for WallState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &SctLayerSurface) {}

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 layer_surface: &SctLayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        for out in &mut self.outputs {
            if out.layer_surface.wl_surface() == layer_surface.wl_surface() {
                if cfg.new_size.0 > 0 { out.width  = cfg.new_size.0; }
                if cfg.new_size.1 > 0 { out.height = cfg.new_size.1; }
                out.configured = true;
                tracing::debug!("wall configure [output {}]: {}×{}", out.output_id, out.width, out.height);
                break;
            }
        }
    }
}

impl ShmHandler for WallState { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl wayland_client::Dispatch<wl_region::WlRegion, ()> for WallState {
    fn event(_: &mut Self, _: &wl_region::WlRegion, _: wl_region::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl ProvidesRegistryState for WallState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_compositor!(WallState);
smithay_client_toolkit::delegate_output!(WallState);
smithay_client_toolkit::delegate_layer!(WallState);
smithay_client_toolkit::delegate_shm!(WallState);
smithay_client_toolkit::delegate_registry!(WallState);
