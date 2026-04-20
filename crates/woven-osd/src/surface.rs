//! Wayland layer-shell surface — bottom-center, no keyboard, transparent bg.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler,
            LayerSurface, LayerSurfaceConfigure,
        },
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};

use crate::render::{OSD_H, OSD_W};

pub struct OsdSurface {
    queue: EventQueue<OsdState>,
    state: OsdState,
}

impl OsdSurface {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env().context("osd: wayland connect")?;
        let (globals, queue) = registry_queue_init::<OsdState>(&conn).context("osd: registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("compositor")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("layer-shell")?;
        let shm         = Shm::bind(&globals, &qh).context("shm")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = OsdState {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            surface:      None,
            pool:          None,
            configured:   false,
        };

        let mut s = Self { queue, state };
        s.queue.roundtrip(&mut s.state).context("osd: roundtrip")?;
        Ok(s)
    }

    pub fn dispatch(&mut self) -> Result<()> {
        use std::os::unix::io::AsRawFd;
        use rustix::fd::AsFd;
        use rustix::event::{PollFd, PollFlags, poll};
        use rustix::time::Timespec;

        if let Err(e) = self.queue.flush() { tracing::debug!("osd flush: {e}"); }
        if let Some(guard) = self.queue.prepare_read() {
            let raw  = self.queue.as_fd().as_raw_fd();
            let bfd  = unsafe { rustix::fd::BorrowedFd::borrow_raw(raw) };
            let mut pfd = PollFd::new(&bfd, PollFlags::IN);
            let ts   = Timespec { tv_sec: 0, tv_nsec: 16_000_000 }; // ~16ms tick
            let ready = poll(std::slice::from_mut(&mut pfd), Some(&ts)).unwrap_or(0);
            if ready > 0 { let _ = guard.read(); } else { drop(guard); }
        }
        self.queue.dispatch_pending(&mut self.state).context("osd dispatch")?;
        Ok(())
    }

    pub fn present(&mut self, pixels: &[u8]) -> Result<()> {
        let s = &mut self.state;
        let Some(ref ls)       = s.surface else { return Ok(()); };
        let Some(ref mut pool) = s.pool    else { return Ok(()); };

        let (w, h) = (OSD_W, OSD_H);
        let stride = w * 4;
        let (buffer, canvas) = pool
            .create_buffer(w as i32, h as i32, stride as i32, wl_shm::Format::Argb8888)
            .context("osd: create_buffer")?;
        let n = canvas.len().min(pixels.len());
        canvas[..n].copy_from_slice(&pixels[..n]);
        let surf = ls.wl_surface();
        buffer.attach_to(surf).ok();
        surf.damage_buffer(0, 0, w as i32, h as i32);
        ls.commit();
        Ok(())
    }

    pub fn configured(&self) -> bool { self.state.configured }
}

// ── Inner state ───────────────────────────────────────────────────────────────

struct OsdState {
    registry:     RegistryState,
    compositor:   CompositorState,
    output_state: OutputState,
    seat_state:   SeatState,
    shm:          Shm,
    layer_shell:  LayerShell,
    surface:      Option<LayerSurface>,
    pool:          Option<SlotPool>,
    configured:   bool,
}

impl OsdState {
    fn create_surface(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        if self.surface.is_some() { return; }
        let wl_surf = self.compositor.create_surface(qh);
        let ls = self.layer_shell.create_layer_surface(
            qh, wl_surf, Layer::Overlay, Some("woven-osd"), Some(output),
        );
        ls.set_anchor(Anchor::BOTTOM);
        ls.set_margin(0, 0, 50, 0);
        ls.set_size(OSD_W, OSD_H);
        ls.set_keyboard_interactivity(KeyboardInteractivity::None);
        ls.commit();

        let pool = SlotPool::new(OSD_W as usize * OSD_H as usize * 4 * 4, &self.shm)
            .expect("osd: pool failed");
        self.surface = Some(ls);
        self.pool    = Some(pool);
    }
}

// ── Handler impls ─────────────────────────────────────────────────────────────

impl CompositorHandler for OsdState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for OsdState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.create_surface(qh, &output);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for OsdState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _: Capability) {}
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _: Capability) {}
}

impl LayerShellHandler for OsdState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {}
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &LayerSurface, _: LayerSurfaceConfigure, _: u32) {
        self.configured = true;
    }
}

impl ShmHandler for OsdState { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl ProvidesRegistryState for OsdState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_compositor!(OsdState);
smithay_client_toolkit::delegate_output!(OsdState);
smithay_client_toolkit::delegate_seat!(OsdState);
smithay_client_toolkit::delegate_layer!(OsdState);
smithay_client_toolkit::delegate_shm!(OsdState);
smithay_client_toolkit::delegate_registry!(OsdState);
