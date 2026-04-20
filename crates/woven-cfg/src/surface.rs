//! Wayland layer-shell surface for woven-cfg.
//! Full-screen overlay with keyboard-exclusive input, centered dialog rendered by panel.

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyboardHandler, KeyEvent, Modifiers, RawModifiers, RepeatInfo},
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
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, EventQueue, QueueHandle,
};
use xkeysym::key;

use crate::panel::{Input, Panel};

// ── Public surface ────────────────────────────────────────────────────────────

pub struct CfgSurface {
    queue: EventQueue<CfgState>,
    state: CfgState,
}

impl CfgSurface {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env().context("cfg: Wayland connect")?;
        let (globals, queue) = registry_queue_init::<CfgState>(&conn).context("cfg: registry")?;
        let qh = queue.handle();

        let compositor  = CompositorState::bind(&globals, &qh).context("compositor missing")?;
        let layer_shell = LayerShell::bind(&globals, &qh).context("layer-shell missing")?;
        let shm         = Shm::bind(&globals, &qh).context("shm missing")?;
        let seat_state  = SeatState::new(&globals, &qh);

        let state = CfgState {
            registry:      RegistryState::new(&globals),
            compositor,
            output_state:  OutputState::new(&globals, &qh),
            seat_state,
            shm,
            layer_shell,
            surface:       None,
            pool:          None,
            pointer:       None,
            keyboard:      None,
            width:         0,
            height:        0,
            configured:    false,
            should_close:  false,
            mouse_x:       0.0,
            mouse_y:       0.0,
            modifiers:     TrackedMods::default(),
            panel:         Panel::new(),
            pending_input: vec![],
            dirty:         true,
        };

        let mut s = Self { queue, state };
        s.queue.roundtrip(&mut s.state).context("cfg: roundtrip")?;
        Ok(s)
    }

    pub fn tick(&mut self) -> Result<bool> {
        use std::os::unix::io::AsRawFd;
        use rustix::fd::AsFd;
        use rustix::event::{PollFd, PollFlags, poll};
        use rustix::time::Timespec;

        if let Err(e) = self.queue.flush() { tracing::debug!("cfg flush: {e}"); }
        if let Some(guard) = self.queue.prepare_read() {
            let raw  = self.queue.as_fd().as_raw_fd();
            let bfd  = unsafe { rustix::fd::BorrowedFd::borrow_raw(raw) };
            let mut pfd = PollFd::new(&bfd, PollFlags::IN);
            let ts   = Timespec { tv_sec: 0, tv_nsec: 5_000_000 };
            let ready = poll(std::slice::from_mut(&mut pfd), Some(&ts)).unwrap_or(0);
            if ready > 0 { let _ = guard.read(); } else { drop(guard); }
        }
        self.queue.dispatch_pending(&mut self.state).context("cfg dispatch")?;

        // Process collected inputs
        let inputs = std::mem::take(&mut self.state.pending_input);
        for inp in inputs {
            self.state.panel.handle(inp);
            self.state.dirty = true;
        }

        self.state.panel.tick_status();

        if self.state.panel.should_close || self.state.should_close { return Ok(true); }

        if (self.state.configured || self.state.width > 0)
            && (self.state.dirty || self.state.panel.dirty) {
            self.state.dirty        = false;
            self.state.panel.dirty  = false;
            self.repaint()?;
        }
        Ok(false)
    }

    fn repaint(&mut self) -> Result<()> {
        let s = &mut self.state;
        let Some(ref ls)       = s.surface  else { return Ok(()); };
        let Some(ref mut pool) = s.pool     else { return Ok(()); };
        let w = s.width;
        let h = s.height;
        if w == 0 || h == 0 { return Ok(()); }

        let pixels = s.panel.render(w, h);
        let stride = w * 4;
        let (buffer, canvas) = pool
            .create_buffer(w as i32, h as i32, stride as i32, wl_shm::Format::Argb8888)
            .context("cfg: create_buffer")?;
        let n = canvas.len().min(pixels.len());
        canvas[..n].copy_from_slice(&pixels[..n]);
        let surf = ls.wl_surface();
        buffer.attach_to(surf).ok();
        surf.damage_buffer(0, 0, w as i32, h as i32);
        ls.commit();
        Ok(())
    }
}

// ── Modifier tracking ─────────────────────────────────────────────────────────

#[derive(Default, Clone)]
struct TrackedMods {
    shift: bool,
    ctrl:  bool,
    super_: bool,
    alt:   bool,
}

impl TrackedMods {
    fn to_sway_prefix(&self) -> String {
        let mut s = String::new();
        if self.super_ { s.push_str("$mod+"); }
        if self.ctrl   { s.push_str("Ctrl+"); }
        if self.alt    { s.push_str("Mod1+"); }
        if self.shift  { s.push_str("Shift+"); }
        s
    }
}

// ── Inner state ───────────────────────────────────────────────────────────────

struct CfgState {
    registry:      RegistryState,
    compositor:    CompositorState,
    output_state:  OutputState,
    seat_state:    SeatState,
    shm:           Shm,
    layer_shell:   LayerShell,
    surface:       Option<LayerSurface>,
    pool:          Option<SlotPool>,
    pointer:       Option<ThemedPointer>,
    keyboard:      Option<wl_keyboard::WlKeyboard>,
    width:         u32,
    height:        u32,
    configured:    bool,
    should_close:  bool,
    mouse_x:       f32,
    mouse_y:       f32,
    modifiers:     TrackedMods,
    panel:         Panel,
    pending_input: Vec<Input>,
    dirty:         bool,
}

impl CfgState {
    fn create_surface(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        if self.surface.is_some() { return; }
        let wl_surf = self.compositor.create_surface(qh);
        let ls = self.layer_shell.create_layer_surface(
            qh, wl_surf, Layer::Overlay, Some("woven-cfg"), Some(output),
        );
        ls.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        ls.set_exclusive_zone(-1);
        ls.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        ls.set_size(0, 0);
        ls.commit();

        let pool = SlotPool::new(3840 * 2160 * 4, &self.shm).expect("cfg: pool failed");
        self.surface = Some(ls);
        self.pool    = Some(pool);
    }

    fn keysym_to_sway_name(raw: u32) -> Option<String> {
        Some(match raw {
            key::Return | key::KP_Enter => "Return".into(),
            key::Tab                    => "Tab".into(),
            key::Escape                 => "Escape".into(),
            key::space                  => "space".into(),
            key::BackSpace              => "BackSpace".into(),
            key::Delete                 => "Delete".into(),
            key::Left                   => "Left".into(),
            key::Right                  => "Right".into(),
            key::Up                     => "Up".into(),
            key::Down                   => "Down".into(),
            key::Home                   => "Home".into(),
            key::End                    => "End".into(),
            key::Page_Up                => "Page_Up".into(),
            key::Page_Down              => "Page_Down".into(),
            key::F1  => "F1".into(),  key::F2  => "F2".into(),
            key::F3  => "F3".into(),  key::F4  => "F4".into(),
            key::F5  => "F5".into(),  key::F6  => "F6".into(),
            key::F7  => "F7".into(),  key::F8  => "F8".into(),
            key::F9  => "F9".into(),  key::F10 => "F10".into(),
            key::F11 => "F11".into(), key::F12 => "F12".into(),
            // XF86 media keys
            0x1008ff11 => "XF86AudioLowerVolume".into(),
            0x1008ff13 => "XF86AudioRaiseVolume".into(),
            0x1008ff12 => "XF86AudioMute".into(),
            0x1008ff14 => "XF86AudioPlay".into(),
            0x1008ff17 => "XF86AudioNext".into(),
            0x1008ff16 => "XF86AudioPrev".into(),
            0x1008ff02 => "XF86MonBrightnessUp".into(),
            0x1008ff03 => "XF86MonBrightnessDown".into(),
            0x1008ff1b => "XF86Search".into(),
            // Print
            key::Print  => "Print".into(),
            key::comma  => "comma".into(),
            key::period => "period".into(),
            key::minus  => "minus".into(),
            key::equal  => "equal".into(),
            key::grave  => "grave".into(),
            key::bracketleft  => "bracketleft".into(),
            key::bracketright => "bracketright".into(),
            key::slash        => "slash".into(),
            key::backslash    => "backslash".into(),
            key::semicolon    => "semicolon".into(),
            key::apostrophe   => "apostrophe".into(),
            // Letters/digits: use utf8
            _ => return None,
        })
    }
}

// ── Handler impls ─────────────────────────────────────────────────────────────

impl CompositorHandler for CfgState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for CfgState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.create_surface(qh, &output);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for CfgState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>,
                      seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kb) => self.keyboard = Some(kb),
                Err(e) => tracing::warn!("cfg: keyboard: {e}"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            let cs = self.compositor.create_surface(qh);
            if let Ok(p) = self.seat_state.get_pointer_with_theme(
                qh, &seat, self.shm.wl_shm(), cs, ThemeSpec::System) {
                self.pointer = Some(p);
            }
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>,
                         _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard { self.keyboard = None; }
        if cap == Capability::Pointer  { self.pointer  = None; }
    }
}

impl KeyboardHandler for CfgState {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface,
             _: u32, _: &[u32], _: &[smithay_client_toolkit::seat::keyboard::Keysym]) {}

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>,
             _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}

    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.handle_key(event, false);
    }

    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                  _: &wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        self.handle_key(event, true);
    }

    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>,
                   _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}

    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>,
                        _: &wl_keyboard::WlKeyboard, _: u32, mods: Modifiers, _: RawModifiers, _: u32) {
        self.modifiers.shift  = mods.shift;
        self.modifiers.ctrl   = mods.ctrl;
        self.modifiers.super_ = mods.logo;
        self.modifiers.alt    = mods.alt;
    }

    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>,
                          _: &wl_keyboard::WlKeyboard, _: RepeatInfo) {}
}

impl CfgState {
    fn handle_key(&mut self, event: KeyEvent, repeat: bool) {
        // Check if we're in keybind capture mode
        let capturing = self.panel.sway_edit.as_ref()
            .map_or(false, |e| e.key.capturing);

        if capturing {
            let raw = event.keysym.raw();
            // Skip pure modifier keys
            match raw {
                key::Shift_L | key::Shift_R | key::Control_L | key::Control_R |
                key::Alt_L   | key::Alt_R   | key::Super_L   | key::Super_R   |
                key::Meta_L  | key::Meta_R  => return,
                _ => {}
            }
            let key_name = if let Some(name) = Self::keysym_to_sway_name(raw) {
                name
            } else if let Some(ref s) = event.utf8 {
                s.to_lowercase()
            } else {
                format!("{:x}", raw)
            };
            let combo = format!("{}{}", self.modifiers.to_sway_prefix(), key_name);
            self.pending_input.push(Input::KeyCombo { key: combo });
            return;
        }

        // Normal mode
        let ctrl = self.modifiers.ctrl;
        match event.keysym.raw() {
            key::Escape    => self.pending_input.push(Input::Escape),
            key::Return | key::KP_Enter => self.pending_input.push(Input::Enter),
            key::Tab       => self.pending_input.push(Input::Tab),
            key::BackSpace => {
                if ctrl { self.pending_input.push(Input::CtrlBackspace); }
                else    { self.pending_input.push(Input::Backspace); }
            }
            key::Left      => self.pending_input.push(Input::Left),
            key::Right     => self.pending_input.push(Input::Right),
            key::Up        => self.pending_input.push(Input::Up),
            key::Down      => self.pending_input.push(Input::Down),
            _ => {
                if let Some(ref s) = event.utf8 {
                    for ch in s.chars() {
                        if !ch.is_control() {
                            self.pending_input.push(Input::Char(ch));
                        }
                    }
                }
            }
        }
    }
}

impl PointerHandler for CfgState {
    fn pointer_frame(&mut self, conn: &Connection, _: &QueueHandle<Self>,
                     _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } => {
                    if let Some(p) = &self.pointer {
                        let _ = p.set_cursor(conn, CursorIcon::Default);
                    }
                    self.mouse_x = ev.position.0 as f32;
                    self.mouse_y = ev.position.1 as f32;
                }
                PointerEventKind::Motion { .. } => {
                    self.mouse_x = ev.position.0 as f32;
                    self.mouse_y = ev.position.1 as f32;
                }
                PointerEventKind::Press { button, .. } if button == 272 => {
                    self.pending_input.push(Input::Click(self.mouse_x, self.mouse_y));
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let dy = if vertical.discrete != 0 {
                        vertical.discrete as f32
                    } else {
                        (vertical.absolute / 15.0) as f32
                    };
                    if dy != 0.0 { self.pending_input.push(Input::Scroll(dy)); }
                }
                _ => {}
            }
        }
    }
}

impl LayerShellHandler for CfgState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.should_close = true;
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>,
                 _: &LayerSurface, cfg: LayerSurfaceConfigure, _: u32) {
        if cfg.new_size.0 > 0 { self.width  = cfg.new_size.0; }
        if cfg.new_size.1 > 0 { self.height = cfg.new_size.1; }
        self.configured = true;
        self.dirty      = true;
    }
}

impl ShmHandler for CfgState { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }

impl ProvidesRegistryState for CfgState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_compositor!(CfgState);
smithay_client_toolkit::delegate_output!(CfgState);
smithay_client_toolkit::delegate_seat!(CfgState);
smithay_client_toolkit::delegate_keyboard!(CfgState);
smithay_client_toolkit::delegate_pointer!(CfgState);
smithay_client_toolkit::delegate_layer!(CfgState);
smithay_client_toolkit::delegate_shm!(CfgState);
smithay_client_toolkit::delegate_registry!(CfgState);
