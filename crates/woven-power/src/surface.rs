//! Wayland fullscreen overlay surface for woven-power.
//! Uses KeyboardInteractivity::Exclusive to grab all input.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyboardHandler, KeyEvent, Keysym, Modifiers, RawModifiers, RepeatInfo},
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemedPointer, ThemeSpec},
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
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_region, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};

use crate::panel::{ClickZone, Panel, Screen};

pub struct PowerSurface {
    queue: EventQueue<State>,
    state: State,
}

impl PowerSurface {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env().context("power: Wayland connect")?;
        let (globals, queue) = registry_queue_init::<State>(&conn).context("power: registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("power: compositor")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("power: layer-shell")?;
        let shm         = Shm::bind(&globals, &qh).context("power: shm")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = State {
            registry:     RegistryState::new(&globals),
            compositor,
            output_state: OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            surface:      None,
            pool:         None,
            sw: 0, sh: 0,
            configured:   false,
            should_close: false,
            mouse_x: 0.0, mouse_y: 0.0,
            pressed:      false,
            pointer:      None,
            click_zones:  Vec::new(),
            panel:        Panel::new(),
            dirty:        true,
        };

        let mut s = Self { queue, state };
        s.queue.roundtrip(&mut s.state).context("power: roundtrip")?;
        Ok(s)
    }

    pub fn tick(&mut self) -> Result<bool> {
        use std::os::unix::io::AsRawFd;
        use rustix::fd::AsFd;
        use rustix::event::{PollFd, PollFlags, poll};
        use rustix::time::Timespec;

        if let Err(e) = self.queue.flush() { tracing::debug!("power flush: {e}"); }
        if let Some(guard) = self.queue.prepare_read() {
            let raw  = self.queue.as_fd().as_raw_fd();
            let bfd  = unsafe { rustix::fd::BorrowedFd::borrow_raw(raw) };
            let mut pfd = PollFd::new(&bfd, PollFlags::IN);
            let ts   = Timespec { tv_sec: 0, tv_nsec: 5_000_000 };
            let ready = poll(std::slice::from_mut(&mut pfd), Some(&ts)).unwrap_or(0);
            if ready > 0 { let _ = guard.read(); } else { drop(guard); }
        }
        self.queue.dispatch_pending(&mut self.state).context("power dispatch")?;

        if self.state.should_close { return Ok(true); }

        if self.state.configured && self.state.dirty {
            self.state.dirty = false;
            self.repaint()?;
        }
        Ok(false)
    }

    fn repaint(&mut self) -> Result<()> {
        let s = &mut self.state;
        if s.sw == 0 || s.sh == 0 { return Ok(()); }
        let Some(ref ls)       = s.surface else { return Ok(()); };
        let Some(ref mut pool) = s.pool    else { return Ok(()); };

        let (pixels, zones) = s.panel.render(s.sw, s.sh);
        s.click_zones = zones;

        let stride = s.sw * 4;
        let (buffer, canvas) = pool
            .create_buffer(s.sw as i32, s.sh as i32, stride as i32, wl_shm::Format::Argb8888)
            .context("power: create_buffer")?;

        let n = canvas.len().min(pixels.len());
        canvas[..n].copy_from_slice(&pixels[..n]);

        let surf = ls.wl_surface();
        buffer.attach_to(surf).ok();
        surf.damage_buffer(0, 0, s.sw as i32, s.sh as i32);
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
    surface:      Option<LayerSurface>,
    pool:         Option<SlotPool>,
    sw: u32, sh: u32,
    configured:   bool,
    should_close: bool,
    mouse_x: f64, mouse_y: f64,
    pressed:      bool,
    pointer:      Option<ThemedPointer>,
    click_zones:  Vec<ClickZone>,
    panel:        Panel,
    dirty:        bool,
}

impl State {
    fn create_surface(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        if self.surface.is_some() { return; }

        let surface = self.compositor.create_surface(qh);
        let ls = self.layer_shell.create_layer_surface(
            qh, surface, Layer::Overlay, Some("woven-power"), Some(output),
        );
        // Full-screen: anchor all sides, let compositor fill
        ls.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        ls.set_exclusive_zone(-1);
        ls.set_size(0, 0);
        ls.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        ls.commit();

        // Generous pool: 1920×1080×4 = 8MB
        let pool = SlotPool::new(1920 * 1080 * 4, &self.shm).expect("power: pool");
        self.surface = Some(ls);
        self.pool    = Some(pool);
    }

    fn handle_key(&mut self, sym: Keysym) -> bool {
        let close = match sym {
            Keysym::Escape => self.panel.key_escape(),
            Keysym::Return | Keysym::KP_Enter => self.panel.key_enter(),
            Keysym::Left  | Keysym::h => { self.panel.nav_prev(); false }
            Keysym::Right | Keysym::l => { self.panel.nav_next(); false }
            Keysym::Up    | Keysym::k => { self.panel.nav_up();   false }
            Keysym::Down  | Keysym::j => { self.panel.nav_down(); false }
            Keysym::_1 => self.panel.select_number(0),
            Keysym::_2 => self.panel.select_number(1),
            Keysym::_3 => self.panel.select_number(2),
            Keysym::_4 => self.panel.select_number(3),
            Keysym::_5 => self.panel.select_number(4),
            Keysym::_6 => self.panel.select_number(5),
            _ => false,
        };
        self.dirty = true;
        close
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
                qh, &seat, self.shm.wl_shm(), cs, ThemeSpec::System) {
                self.pointer = Some(p);
            }
        }
        if cap == Capability::Keyboard {
            let _ = self.seat_state.get_keyboard(qh, &seat, None);
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>,
                         _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer { self.pointer = None; }
    }
}

impl KeyboardHandler for State {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard,
             _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard,
             _: &wl_surface::WlSurface, _: u32) {}

    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        if self.handle_key(event.keysym) { self.should_close = true; }
    }

    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                   _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}

    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                  _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        // Allow repeat for nav keys
        match event.keysym {
            Keysym::Left | Keysym::Right | Keysym::Up | Keysym::Down |
            Keysym::h | Keysym::j | Keysym::k | Keysym::l => {
                if self.handle_key(event.keysym) { self.should_close = true; }
            }
            _ => {}
        }
    }

    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>,
                        _: &wl_keyboard::WlKeyboard, _: u32, _: Modifiers,
                        _: RawModifiers, _: u32) {}

    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>,
                          _: &wl_keyboard::WlKeyboard, _: RepeatInfo) {}
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>,
                     _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            match ev.kind {
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
                        let zones = self.click_zones.clone();
                        for z in &zones {
                            if x >= z.x0 && x <= z.x1 && y >= z.y0 && y <= z.y1 {
                                let close = match z.action_idx {
                                    100 => { // Cancel
                                        self.panel.screen   = Screen::Main;
                                        self.panel.selected = 0;
                                        false
                                    }
                                    101 => { // Confirm
                                        self.panel.key_enter()
                                    }
                                    i => {
                                        self.panel.selected = i;
                                        self.panel.key_enter()
                                    }
                                };
                                self.dirty = true;
                                if close { self.should_close = true; }
                                break;
                            }
                        }
                        // Click outside dialog = dismiss confirm / close main
                        if zones.is_empty() { self.should_close = true; }
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
                if cfg.new_size.0 > 0 { self.sw = cfg.new_size.0; }
                if cfg.new_size.1 > 0 { self.sh = cfg.new_size.1; }
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
smithay_client_toolkit::delegate_keyboard!(State);
smithay_client_toolkit::delegate_layer!(State);
smithay_client_toolkit::delegate_shm!(State);
smithay_client_toolkit::delegate_registry!(State);
