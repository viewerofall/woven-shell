//! Wayland layer-shell overlay surface for woven-pick.
//! Layer::Overlay, keyboard exclusive, full-screen on focused output.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyboardHandler, KeyEvent, Modifiers, RawModifiers, RepeatInfo},
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemedPointer, ThemeSpec},
    },
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
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_region, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};

use crate::picker::Picker;

// ─── public surface ───────────────────────────────────────────────────────────

pub struct PickSurface {
    queue: EventQueue<PickState>,
    state: PickState,
}

impl PickSurface {
    pub fn new(picker: Picker) -> Result<Self> {
        let conn = Connection::connect_to_env()
            .context("pick: failed to connect to Wayland display")?;
        let (globals, queue) = registry_queue_init::<PickState>(&conn)
            .context("pick: failed to init Wayland registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("pick: wl_compositor missing")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("pick: wlr-layer-shell missing")?;
        let shm         = Shm::bind(&globals, &qh).context("pick: wl_shm missing")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = PickState {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            surface:      None,
            pointer:      None,
            keyboard:     None,
            width:        0,
            height:       0,
            configured:   false,
            picker,
            mouse_x:      0.0,
            mouse_y:      0.0,
        };

        let mut s = Self { queue, state };
        let _ = s.queue.roundtrip(&mut s.state);
        Ok(s)
    }

    pub fn configured(&self) -> bool { self.state.configured }
    pub fn size(&self)        -> (u32, u32) { (self.state.width, self.state.height) }
    pub fn should_close(&self) -> bool { self.state.picker.should_close }
    pub fn apply_path(&self)  -> Option<std::path::PathBuf> { self.state.picker.apply_path.clone() }
    pub fn render_frame(&mut self, w: u32, h: u32) -> Vec<u8> { self.state.picker.render(w, h) }

    pub fn needs_render(&self) -> bool { self.state.picker.is_animating() || self.state.picker.dirty }

    pub fn ensure_surface(&mut self) {
        let qh = self.queue.handle();
        self.state.create_surface(&qh);
    }

    pub fn dispatch(&mut self) -> Result<()> {
        if let Err(e) = self.queue.flush() {
            tracing::debug!("pick flush: {e}");
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
        self.queue.dispatch_pending(&mut self.state).context("pick dispatch failed")?;
        if !self.state.configured {
            let _ = self.queue.roundtrip(&mut self.state);
        }
        Ok(())
    }

    pub fn present(&mut self, pixels: Vec<u8>) -> Result<()> {
        let s = &mut self.state;
        if !s.configured || s.width == 0 || s.height == 0 { return Ok(()); }
        let (w, h) = (s.width, s.height);
        let surf   = s.surface.as_mut().context("pick: no surface")?;
        let stride = w * 4;
        let (buffer, canvas) = surf.pool
            .create_buffer(w as i32, h as i32, stride as i32, wl_shm::Format::Argb8888)
            .context("pick: create_buffer failed")?;
        let n = canvas.len().min(pixels.len());
        canvas[..n].copy_from_slice(&pixels[..n]);
        let wl = surf.layer.wl_surface();
        buffer.attach_to(wl).ok();
        wl.damage_buffer(0, 0, w as i32, h as i32);
        surf.layer.commit();
        Ok(())
    }
}

// ─── inner state ──────────────────────────────────────────────────────────────

struct PerSurface {
    layer: SctLayerSurface,
    pool:  SlotPool,
}

struct PickState {
    registry:     RegistryState,
    compositor:   CompositorState,
    output_state: OutputState,
    seat_state:   SeatState,
    shm:          Shm,
    layer_shell:  LayerShell,
    surface:      Option<PerSurface>,
    pointer:      Option<ThemedPointer>,
    keyboard:     Option<wl_keyboard::WlKeyboard>,
    width:        u32,
    height:       u32,
    configured:   bool,
    picker:       Picker,
    mouse_x:      f64,
    mouse_y:      f64,
}

impl PickState {
    fn create_surface(&mut self, qh: &QueueHandle<Self>) {
        if self.surface.is_some() { return; }
        let wl_surf = self.compositor.create_surface(qh);
        let layer   = self.layer_shell.create_layer_surface(
            qh, wl_surf, Layer::Overlay, Some("woven-pick"), None,
        );
        layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer.set_exclusive_zone(0);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.set_size(1000, 700);
        layer.set_margin(10, 10, 10, 10);
        layer.commit();

        let pool = SlotPool::new(32 * 1024 * 1024, &self.shm)
            .expect("pick: shm pool failed");

        self.surface = Some(PerSurface { layer, pool });
    }
}

// ─── Smithay handler impls ────────────────────────────────────────────────────

impl CompositorHandler for PickState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for PickState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for PickState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>,
                      seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kb) => { self.keyboard = Some(kb); }
                Err(e) => tracing::warn!("pick: keyboard failed: {e}"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            let cs = self.compositor.create_surface(qh);
            if let Ok(p) = self.seat_state.get_pointer_with_theme(
                qh, &seat, self.shm.wl_shm(), cs, ThemeSpec::System)
            { self.pointer = Some(p); }
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>,
                         _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard { self.keyboard = None; }
        if cap == Capability::Pointer  { self.pointer  = None; }
    }
}

impl KeyboardHandler for PickState {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[smithay_client_toolkit::seat::keyboard::Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}

    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        let (w, h) = (self.width, self.height);
        self.picker.handle_key(event.keysym.raw(), event.utf8.as_deref(), w, h);
        self.picker.mark_dirty();
    }

    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                   _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                  _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        let (w, h) = (self.width, self.height);
        self.picker.handle_key(event.keysym.raw(), event.utf8.as_deref(), w, h);
    }
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>,
                        _: &wl_keyboard::WlKeyboard, _: u32, _: Modifiers, _: RawModifiers, _: u32) {}
    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>,
                          _: &wl_keyboard::WlKeyboard, _: RepeatInfo) {}
}

impl PointerHandler for PickState {
    fn pointer_frame(&mut self, conn: &Connection, _: &QueueHandle<Self>,
                     _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        let (w, h) = (self.width, self.height);
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } => {
                    if let Some(p) = &self.pointer {
                        let _ = p.set_cursor(conn, smithay_client_toolkit::seat::pointer::CursorIcon::Default);
                    }
                    self.mouse_x = ev.position.0;
                    self.mouse_y = ev.position.1;
                    self.picker.handle_pointer_move(self.mouse_x, self.mouse_y, w);
                    self.picker.mark_dirty();
                }
                PointerEventKind::Motion { .. } => {
                    self.mouse_x = ev.position.0;
                    self.mouse_y = ev.position.1;
                    self.picker.handle_pointer_move(self.mouse_x, self.mouse_y, w);
                }
                PointerEventKind::Press { button, .. } => {
                    if button == 272 { // left
                        self.picker.handle_click(self.mouse_x, self.mouse_y, w, h);
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let dy = if vertical.discrete != 0 {
                        vertical.discrete as f64
                    } else {
                        vertical.absolute / 15.0
                    };
                    if dy != 0.0 {
                        self.picker.handle_scroll(dy, w, h);
                        self.picker.mark_dirty();
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.picker.handle_pointer_move(-1.0, -1.0, w);
                }
                _ => {}
            }
        }
    }
}

impl LayerShellHandler for PickState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &SctLayerSurface) {
        self.picker.should_close = true;
    }

    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>,
                 _: &SctLayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        if cfg.new_size.0 > 0 { self.width  = cfg.new_size.0; }
        if cfg.new_size.1 > 0 { self.height = cfg.new_size.1; }
        if !self.configured {
            self.configured = true;
            tracing::info!("pick: configured {}×{}", self.width, self.height);
        }
        let _ = qh;
    }
}

impl ShmHandler for PickState { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl wayland_client::Dispatch<wl_region::WlRegion, ()> for PickState {
    fn event(_: &mut Self, _: &wl_region::WlRegion, _: wl_region::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl ProvidesRegistryState for PickState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_compositor!(PickState);
smithay_client_toolkit::delegate_output!(PickState);
smithay_client_toolkit::delegate_seat!(PickState);
smithay_client_toolkit::delegate_keyboard!(PickState);
smithay_client_toolkit::delegate_pointer!(PickState);
smithay_client_toolkit::delegate_layer!(PickState);
smithay_client_toolkit::delegate_shm!(PickState);
smithay_client_toolkit::delegate_registry!(PickState);
