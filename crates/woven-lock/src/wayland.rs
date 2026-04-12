//! Wayland layer-shell surface for woven-lock.
//! Layer::Overlay on ALL outputs with KeyboardInteractivity::Exclusive.
//! Captures keyboard input for password entry.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
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
    Connection, EventQueue, Proxy, QueueHandle,
};

// ─── Input events sent to main loop ──────────────────────────────────────────

pub enum LockInput {
    Char(char),
    Backspace,
    Enter,
    Escape,
}

// ─── Per-output surface ──────────────────────────────────────────────────────

struct PerOutput {
    output_id:     u32,
    layer_surface: SctLayerSurface,
    pool:          SlotPool,
    width:         u32,
    height:        u32,
    configured:    bool,
    needs_opaque:  bool,
}

// ─── Public surface handle ───────────────────────────────────────────────────

pub struct LockSurface {
    queue: EventQueue<LockState>,
    state: LockState,
}

impl LockSurface {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env()
            .context("lock: failed to connect to Wayland display")?;
        let (globals, queue) = registry_queue_init::<LockState>(&conn)
            .context("lock: failed to init Wayland registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("lock: wl_compositor missing")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("lock: wlr-layer-shell missing")?;
        let shm         = Shm::bind(&globals, &qh).context("lock: wl_shm missing")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = LockState {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            outputs:      Vec::new(),
            pointer:      None,
            keyboard:     None,
            input_queue:  Vec::new(),
        };

        let mut s = Self { queue, state };
        let _ = s.queue.roundtrip(&mut s.state);
        Ok(s)
    }

    pub fn configured_count(&self) -> usize {
        self.state.outputs.iter().filter(|o| o.configured).count()
    }

    /// Drain pending input events from the keyboard handler.
    pub fn drain_input(&mut self) -> Vec<LockInput> {
        std::mem::take(&mut self.state.input_queue)
    }

    pub fn dispatch(&mut self) -> Result<()> {
        if let Err(e) = self.queue.flush() {
            tracing::debug!("lock flush: {e}");
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
        self.queue.dispatch_pending(&mut self.state).context("lock dispatch failed")?;
        // roundtrip if any outputs not yet configured
        if self.state.outputs.iter().any(|o| !o.configured) {
            let _ = self.queue.roundtrip(&mut self.state);
        }
        Ok(())
    }

    /// Paint all configured outputs with the given pixel-generating function.
    /// paint_fn receives (width, height) and must return BGRA pixels.
    pub fn present_all<F: FnMut(u32, u32) -> Vec<u8>>(&mut self, mut paint_fn: F) -> Result<()> {
        for out in &mut self.state.outputs {
            if !out.configured || out.width == 0 || out.height == 0 { continue; }

            // set opaque region once after configure — tells compositor nothing behind us is visible
            if out.needs_opaque {
                if let Ok(region) = Region::new(&self.state.compositor) {
                    region.add(0, 0, out.width as i32, out.height as i32);
                    out.layer_surface.wl_surface().set_opaque_region(Some(region.wl_region()));
                }
                out.needs_opaque = false;
            }

            let pixels = paint_fn(out.width, out.height);
            let stride = out.width * 4;
            let (buffer, canvas) = out.pool
                .create_buffer(
                    out.width as i32, out.height as i32,
                    stride as i32, wl_shm::Format::Argb8888,
                )
                .context("lock: create_buffer failed")?;
            let n = canvas.len().min(pixels.len());
            canvas[..n].copy_from_slice(&pixels[..n]);
            let surf = out.layer_surface.wl_surface();
            buffer.attach_to(surf).ok();
            surf.damage_buffer(0, 0, out.width as i32, out.height as i32);
            out.layer_surface.commit();
        }
        Ok(())
    }

    /// Get all configured output sizes.
    pub fn output_sizes(&self) -> Vec<(u32, u32)> {
        self.state.outputs.iter()
            .filter(|o| o.configured && o.width > 0 && o.height > 0)
            .map(|o| (o.width, o.height))
            .collect()
    }
}

// ─── Inner state ─────────────────────────────────────────────────────────────

struct LockState {
    registry:     RegistryState,
    compositor:   CompositorState,
    output_state: OutputState,
    seat_state:   SeatState,
    shm:          Shm,
    layer_shell:  LayerShell,
    outputs:      Vec<PerOutput>,
    pointer:      Option<ThemedPointer>,
    keyboard:     Option<wl_keyboard::WlKeyboard>,
    input_queue:  Vec<LockInput>,
}

impl LockState {
    fn add_output(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        let output_id = output.id().protocol_id();
        if self.outputs.iter().any(|o| o.output_id == output_id) { return; }

        let surface       = self.compositor.create_surface(qh);
        let layer_surface = self.layer_shell.create_layer_surface(
            qh, surface, Layer::Overlay,
            Some("woven-lock"), Some(output),
        );

        layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_size(0, 0);
        layer_surface.commit();

        // 64 MB pool: covers 4K double-buffered
        let pool = match SlotPool::new(64 * 1024 * 1024, &self.shm) {
            Ok(p)  => p,
            Err(e) => { tracing::warn!("lock: shm pool failed for output {output_id}: {e}"); return; }
        };

        tracing::info!("lock: added surface for output {output_id}");
        self.outputs.push(PerOutput {
            output_id,
            layer_surface,
            pool,
            width:        0,
            height:       0,
            configured:   false,
            needs_opaque: false,
        });
    }
}

// ─── Smithay handler impls ───────────────────────────────────────────────────

impl CompositorHandler for LockState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for LockState {
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

impl SeatHandler for LockState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>,
                      seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kb) => { self.keyboard = Some(kb); }
                Err(e) => tracing::warn!("lock: keyboard failed: {e}"),
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

impl KeyboardHandler for LockState {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32,
             _: &[u32], _: &[smithay_client_toolkit::seat::keyboard::Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}

    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.handle_key(event);
    }

    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                   _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}

    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                  _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.handle_key(event);
    }

    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>,
                        _: &wl_keyboard::WlKeyboard, _: u32, _: Modifiers, _: RawModifiers, _: u32) {}
    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>,
                          _: &wl_keyboard::WlKeyboard, _: RepeatInfo) {}
}

impl LockState {
    fn handle_key(&mut self, event: KeyEvent) {
        use xkeysym::Keysym;
        let sym = event.keysym.raw();

        if sym == Keysym::Return.raw() || sym == Keysym::KP_Enter.raw() {
            self.input_queue.push(LockInput::Enter);
        } else if sym == Keysym::BackSpace.raw() {
            self.input_queue.push(LockInput::Backspace);
        } else if sym == Keysym::Escape.raw() {
            self.input_queue.push(LockInput::Escape);
        } else if let Some(ref text) = event.utf8 {
            for ch in text.chars() {
                if !ch.is_control() {
                    self.input_queue.push(LockInput::Char(ch));
                }
            }
        }
    }
}

impl PointerHandler for LockState {
    fn pointer_frame(&mut self, conn: &Connection, _: &QueueHandle<Self>,
                     _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            if let PointerEventKind::Enter { .. } = ev.kind {
                // hide cursor on lock screen — no mouse interaction needed
                if let Some(p) = &self.pointer {
                    let _ = p.set_cursor(conn, smithay_client_toolkit::seat::pointer::CursorIcon::Default);
                }
            }
        }
    }
}

impl LayerShellHandler for LockState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &SctLayerSurface) {}

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 layer_surface: &SctLayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        for out in &mut self.outputs {
            if out.layer_surface.wl_surface() == layer_surface.wl_surface() {
                if cfg.new_size.0 > 0 { out.width  = cfg.new_size.0; }
                if cfg.new_size.1 > 0 { out.height = cfg.new_size.1; }
                out.configured = true;
                out.needs_opaque = true;
                tracing::info!("lock configure [output {}]: {}×{}", out.output_id, out.width, out.height);
                break;
            }
        }
    }
}

impl ShmHandler for LockState { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl wayland_client::Dispatch<wl_region::WlRegion, ()> for LockState {
    fn event(_: &mut Self, _: &wl_region::WlRegion, _: wl_region::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl ProvidesRegistryState for LockState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_compositor!(LockState);
smithay_client_toolkit::delegate_output!(LockState);
smithay_client_toolkit::delegate_seat!(LockState);
smithay_client_toolkit::delegate_keyboard!(LockState);
smithay_client_toolkit::delegate_pointer!(LockState);
smithay_client_toolkit::delegate_layer!(LockState);
smithay_client_toolkit::delegate_shm!(LockState);
smithay_client_toolkit::delegate_registry!(LockState);
