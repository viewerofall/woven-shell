//! Wayland layer-shell surface for the control center popup.
//! Anchored top-right, below the bar. No keyboard handling — use PID toggle to close.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemedPointer, ThemeSpec},
    },
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
    protocol::{wl_output, wl_pointer, wl_region, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};

use crate::panel::{Panel, WIDTH};

pub struct CcSurface {
    queue: EventQueue<State>,
    state: State,
}

impl CcSurface {
    pub fn new(bar_height: u32) -> Result<Self> {
        let conn = Connection::connect_to_env().context("cc: Wayland connect")?;
        let (globals, queue) = registry_queue_init::<State>(&conn).context("cc: registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("cc: compositor")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("cc: layer-shell")?;
        let shm         = Shm::bind(&globals, &qh).context("cc: shm")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = State {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            bar_height,
            surface:      None,
            pool:         None,
            width:        0,
            height:       0,
            configured:   false,
            should_close: false,
            mouse_x:      0.0,
            mouse_y:      0.0,
            pressed:      false,
            pointer:      None,
            panel:        Panel::new(),
            dirty:        true,
        };

        let mut s = Self { queue, state };
        s.queue.roundtrip(&mut s.state).context("cc: roundtrip")?;
        Ok(s)
    }

    /// Returns true when the panel should exit.
    pub fn tick(&mut self) -> Result<bool> {
        use std::os::unix::io::AsRawFd;
        use rustix::fd::AsFd;
        use rustix::event::{PollFd, PollFlags, poll};
        use rustix::time::Timespec;

        if let Err(e) = self.queue.flush() {
            tracing::debug!("cc flush: {e}");
        }
        if let Some(guard) = self.queue.prepare_read() {
            let raw  = self.queue.as_fd().as_raw_fd();
            let bfd  = unsafe { rustix::fd::BorrowedFd::borrow_raw(raw) };
            let mut pfd = PollFd::new(&bfd, PollFlags::IN);
            let ts   = Timespec { tv_sec: 0, tv_nsec: 5_000_000 };
            let ready = poll(std::slice::from_mut(&mut pfd), Some(&ts)).unwrap_or(0);
            if ready > 0 { let _ = guard.read(); } else { drop(guard); }
        }
        self.queue.dispatch_pending(&mut self.state).context("cc dispatch")?;

        if self.state.should_close { return Ok(true); }

        if self.state.configured && self.state.dirty {
            self.state.dirty = false;
            self.repaint()?;
        }
        Ok(false)
    }

    fn repaint(&mut self) -> Result<()> {
        let s = &mut self.state;
        let Some(ref ls)   = s.surface else { return Ok(()); };
        let Some(ref mut pool) = s.pool else { return Ok(()); };

        let (pixels, height) = s.panel.render();
        let width  = WIDTH;
        let stride = width * 4;

        // Update surface size if needed
        if width != s.width || height != s.height {
            ls.set_size(width, height);
            s.width  = width;
            s.height = height;
        }

        let (buffer, canvas) = pool
            .create_buffer(width as i32, height as i32, stride as i32, wl_shm::Format::Argb8888)
            .context("cc: create_buffer")?;

        let n = canvas.len().min(pixels.len());
        canvas[..n].copy_from_slice(&pixels[..n]);

        let surf = ls.wl_surface();
        buffer.attach_to(surf).ok();
        surf.damage_buffer(0, 0, width as i32, height as i32);
        ls.commit();
        Ok(())
    }
}

// ── Wayland state ─────────────────────────────────────────────────────────────

struct State {
    registry:     RegistryState,
    compositor:   CompositorState,
    output_state: OutputState,
    seat_state:   SeatState,
    shm:          Shm,
    layer_shell:  LayerShell,
    bar_height:   u32,
    surface:      Option<LayerSurface>,
    pool:         Option<SlotPool>,
    width:        u32,
    height:       u32,
    configured:   bool,
    should_close: bool,
    mouse_x:      f64,
    mouse_y:      f64,
    pressed:      bool,
    pointer:      Option<ThemedPointer>,
    panel:        Panel,
    dirty:        bool,
}

impl State {
    fn create_surface(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        if self.surface.is_some() { return; }

        let surface = self.compositor.create_surface(qh);
        let ls = self.layer_shell.create_layer_surface(
            qh, surface, Layer::Overlay, Some("woven-cc"), Some(output),
        );
        ls.set_anchor(Anchor::TOP | Anchor::RIGHT);
        ls.set_exclusive_zone(-1);
        ls.set_margin(self.bar_height as i32 + 4, 10, 0, 0);
        ls.set_size(WIDTH, 400);
        ls.set_keyboard_interactivity(KeyboardInteractivity::None);
        ls.commit();

        let pool = SlotPool::new(WIDTH as usize * 600 * 4, &self.shm)
            .expect("cc: pool failed");

        self.surface = Some(ls);
        self.pool    = Some(pool);
    }
}

// ── Handler impls ─────────────────────────────────────────────────────────────

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.create_surface(qh, &output);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>,
                      seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer && self.pointer.is_none() {
            let cs = self.compositor.create_surface(qh);
            if let Ok(p) = self.seat_state.get_pointer_with_theme(
                qh, &seat, self.shm.wl_shm(), cs, ThemeSpec::System)
            {
                self.pointer = Some(p);
            }
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>,
                         _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer { self.pointer = None; }
    }
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, conn: &Connection, _: &QueueHandle<Self>,
                     _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } => {
                    if let Some(p) = &self.pointer {
                        let _ = p.set_cursor(conn, CursorIcon::Default);
                    }
                    self.mouse_x = ev.position.0;
                    self.mouse_y = ev.position.1;
                }
                PointerEventKind::Motion { .. } => {
                    self.mouse_x = ev.position.0;
                    self.mouse_y = ev.position.1;
                }
                PointerEventKind::Press { button, .. } if button == 272 => {
                    self.pressed = true;
                }
                PointerEventKind::Release { button, .. } if button == 272 => {
                    if self.pressed {
                        self.pressed = false;
                        let x = self.mouse_x as f32;
                        let y = self.mouse_y as f32;
                        let zones = self.panel.zones.clone();
                        for z in &zones {
                            if x >= z.x0 && x <= z.x1 && y >= z.y0 && y <= z.y1 {
                                let close = self.panel.handle(z.btn);
                                self.dirty = true;
                                if close { self.should_close = true; }
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.should_close = true;
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 ls: &LayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        if let Some(ref s) = self.surface {
            if s.wl_surface() == ls.wl_surface() {
                if cfg.new_size.0 > 0 { self.width  = cfg.new_size.0; }
                if cfg.new_size.1 > 0 { self.height = cfg.new_size.1; }
                self.configured = true;
                self.dirty      = true;
            }
        }
    }
}

impl ShmHandler for State { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl wayland_client::Dispatch<wl_region::WlRegion, ()> for State {
    fn event(_: &mut Self, _: &wl_region::WlRegion, _: wl_region::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_compositor!(State);
smithay_client_toolkit::delegate_output!(State);
smithay_client_toolkit::delegate_seat!(State);
smithay_client_toolkit::delegate_pointer!(State);
smithay_client_toolkit::delegate_layer!(State);
smithay_client_toolkit::delegate_shm!(State);
smithay_client_toolkit::delegate_registry!(State);
