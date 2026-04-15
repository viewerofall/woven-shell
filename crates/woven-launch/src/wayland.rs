//! Wayland layer-shell overlay surface for woven-launch.
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
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};
use xkeysym::key;

// ─── Input events ────────────────────────────────────────────────────────────

pub enum LaunchInput {
    Char(char),
    Backspace,
    Enter,
    Escape,
    Up,
    Down,
    Tab,
    PageUp,
    PageDown,
    Click(f64, f64),
    Scroll(f64),
    MouseMove(f64, f64),
}

// ─── Public surface ──────────────────────────────────────────────────────────

pub struct LaunchSurface {
    queue: EventQueue<LaunchState>,
    state: LaunchState,
}

impl LaunchSurface {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env()
            .context("launch: failed to connect to Wayland display")?;
        let (globals, queue) = registry_queue_init::<LaunchState>(&conn)
            .context("launch: failed to init Wayland registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("wl_compositor missing")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("wlr-layer-shell missing")?;
        let shm         = Shm::bind(&globals, &qh).context("wl_shm missing")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = LaunchState {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            surface:      None,
            pool:         None,
            pointer:      None,
            keyboard:     None,
            width:        0,
            height:       0,
            configured:   false,
            pending_input: Vec::new(),
        };

        let mut s = Self { queue, state };
        let _ = s.queue.roundtrip(&mut s.state);
        Ok(s)
    }

    pub fn ensure_surface(&mut self) {
        let qh = self.queue.handle();
        self.state.create_surface(&qh);
    }

    pub fn configured(&self) -> bool { self.state.configured }
    pub fn size(&self) -> (u32, u32) { (self.state.width, self.state.height) }

    pub fn drain_input(&mut self) -> Vec<LaunchInput> {
        std::mem::take(&mut self.state.pending_input)
    }

    pub fn dispatch(&mut self) -> Result<()> {
        if let Err(e) = self.queue.flush() {
            tracing::debug!("launch flush: {e}");
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
        self.queue.dispatch_pending(&mut self.state).context("launch dispatch failed")?;
        if !self.state.configured {
            let _ = self.queue.roundtrip(&mut self.state);
        }
        Ok(())
    }

    pub fn present(&mut self, pixels: Vec<u8>) -> Result<()> {
        let s = &mut self.state;
        if !s.configured || s.width == 0 || s.height == 0 { return Ok(()); }
        let (w, h) = (s.width, s.height);

        let pool = s.pool.as_mut().context("launch: no shm pool")?;
        let stride = w * 4;
        let (buffer, canvas) = pool
            .create_buffer(w as i32, h as i32, stride as i32, wl_shm::Format::Argb8888)
            .context("launch: create_buffer failed")?;
        let n = canvas.len().min(pixels.len());
        canvas[..n].copy_from_slice(&pixels[..n]);

        let surf = s.surface.as_ref().context("launch: no surface")?;
        let wl = surf.wl_surface();
        buffer.attach_to(wl).ok();
        wl.damage_buffer(0, 0, w as i32, h as i32);
        surf.commit();
        Ok(())
    }
}

// ─── Inner state ─────────────────────────────────────────────────────────────

struct LaunchState {
    registry:      RegistryState,
    compositor:    CompositorState,
    output_state:  OutputState,
    seat_state:    SeatState,
    shm:           Shm,
    layer_shell:   LayerShell,
    surface:       Option<SctLayerSurface>,
    pool:          Option<SlotPool>,
    pointer:       Option<ThemedPointer>,
    keyboard:      Option<wl_keyboard::WlKeyboard>,
    width:         u32,
    height:        u32,
    configured:    bool,
    pending_input: Vec<LaunchInput>,
}

impl LaunchState {
    fn create_surface(&mut self, qh: &QueueHandle<Self>) {
        if self.surface.is_some() { return; }
        let wl_surf = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh, wl_surf, Layer::Overlay, Some("woven-launch"), None,
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.set_size(0, 0);
        layer.commit();

        let pool = SlotPool::new(32 * 1024 * 1024, &self.shm)
            .expect("launch: shm pool failed");

        self.surface = Some(layer);
        self.pool = Some(pool);
    }
}

// ─── Smithay handler impls ───────────────────────────────────────────────────

impl CompositorHandler for LaunchState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for LaunchState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for LaunchState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>,
                      seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kb) => { self.keyboard = Some(kb); }
                Err(e) => tracing::warn!("launch: keyboard failed: {e}"),
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

impl KeyboardHandler for LaunchState {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[smithay_client_toolkit::seat::keyboard::Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}

    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.map_key(event);
    }

    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                   _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}

    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                  _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.map_key(event);
    }

    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>,
                        _: &wl_keyboard::WlKeyboard, _: u32, _: Modifiers, _: RawModifiers, _: u32) {}
    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>,
                          _: &wl_keyboard::WlKeyboard, _: RepeatInfo) {}
}

impl LaunchState {
    fn map_key(&mut self, event: KeyEvent) {
        match event.keysym.raw() {
            key::Escape => self.pending_input.push(LaunchInput::Escape),
            key::Return | key::KP_Enter => self.pending_input.push(LaunchInput::Enter),
            key::BackSpace => self.pending_input.push(LaunchInput::Backspace),
            key::Up => self.pending_input.push(LaunchInput::Up),
            key::Down => self.pending_input.push(LaunchInput::Down),
            key::Tab => self.pending_input.push(LaunchInput::Tab),
            key::Page_Up => self.pending_input.push(LaunchInput::PageUp),
            key::Page_Down => self.pending_input.push(LaunchInput::PageDown),
            _ => {
                if let Some(s) = &event.utf8 {
                    for ch in s.chars() {
                        if !ch.is_control() {
                            self.pending_input.push(LaunchInput::Char(ch));
                        }
                    }
                }
            }
        }
    }
}

impl PointerHandler for LaunchState {
    fn pointer_frame(&mut self, conn: &Connection, _: &QueueHandle<Self>,
                     _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } => {
                    if let Some(p) = &self.pointer {
                        let _ = p.set_cursor(conn, smithay_client_toolkit::seat::pointer::CursorIcon::Default);
                    }
                    self.pending_input.push(LaunchInput::MouseMove(ev.position.0, ev.position.1));
                }
                PointerEventKind::Motion { .. } => {
                    self.pending_input.push(LaunchInput::MouseMove(ev.position.0, ev.position.1));
                }
                PointerEventKind::Press { button, .. } => {
                    if button == 272 { // left click
                        self.pending_input.push(LaunchInput::Click(ev.position.0, ev.position.1));
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let dy = if vertical.discrete != 0 {
                        vertical.discrete as f64
                    } else {
                        vertical.absolute / 15.0
                    };
                    if dy != 0.0 {
                        self.pending_input.push(LaunchInput::Scroll(dy));
                    }
                }
                _ => {}
            }
        }
    }
}

impl LayerShellHandler for LaunchState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &SctLayerSurface) {
        self.pending_input.push(LaunchInput::Escape);
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &SctLayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        if cfg.new_size.0 > 0 { self.width  = cfg.new_size.0; }
        if cfg.new_size.1 > 0 { self.height = cfg.new_size.1; }
        if !self.configured {
            self.configured = true;
            tracing::info!("launch: configured {}x{}", self.width, self.height);
        }
    }
}

impl ShmHandler for LaunchState { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl ProvidesRegistryState for LaunchState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_compositor!(LaunchState);
smithay_client_toolkit::delegate_output!(LaunchState);
smithay_client_toolkit::delegate_seat!(LaunchState);
smithay_client_toolkit::delegate_keyboard!(LaunchState);
smithay_client_toolkit::delegate_pointer!(LaunchState);
smithay_client_toolkit::delegate_layer!(LaunchState);
smithay_client_toolkit::delegate_shm!(LaunchState);
smithay_client_toolkit::delegate_registry!(LaunchState);
