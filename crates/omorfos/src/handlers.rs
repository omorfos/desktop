//! Wayland protocol handlers: what clients ask for, translated into
//! operations on the desktop model.

use omorfos_core::geometry::Size as DomainSize;
use omorfos_core::{Desktop, WindowId};
use smithay::backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state};
use smithay::desktop::{PopupKind, Window};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, get_parent, with_states,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_output, delegate_seat, delegate_shm, delegate_xdg_shell,
};

use crate::compositor::{ClientState, Compositor, Mapped};

impl CompositorHandler for Compositor {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.surfaces
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        #[expect(
            clippy::expect_used,
            reason = "every client is inserted with a `ClientState`"
        )]
        let state = client
            .get_data::<ClientState>()
            .expect("client without state");
        &state.compositor
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        self.windows
            .iter()
            .filter(|mapped| *mapped.toplevel.wl_surface() == root)
            .for_each(|mapped| mapped.window.on_commit());

        self.map_if_ready(surface);

        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface)
            && !popup.is_initial_configure_sent()
        {
            // The only failure is the popup's parent having gone away, in
            // which case there is nothing to configure.
            let _ = popup.send_configure();
        }
    }
}

impl Compositor {
    /// Moves a new window along: answers its first commit with a configure,
    /// and hands it to the desktop once it has content to show.
    fn map_if_ready(&mut self, surface: &WlSurface) {
        let Some(index) = self
            .unmapped
            .iter()
            .position(|unmapped| unmapped.wl_surface() == surface)
        else {
            return;
        };
        if !self.unmapped[index].is_initial_configure_sent() {
            self.unmapped[index].send_configure();
            return;
        }
        if !with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false)
        {
            return;
        }
        let toplevel = self.unmapped.remove(index);
        let window = Window::new_wayland_window(toplevel.clone());
        window.on_commit();
        let (app_id, title) = names(surface);
        let size = window.geometry().size;
        let size = DomainSize::new(size.w.unsigned_abs(), size.h.unsigned_abs());
        let id = self
            .desktop
            .add_window(app_id, title, size, self.pointer_position());
        self.windows.push(Mapped {
            id,
            window,
            toplevel,
        });
    }

    /// Applies an operation to a toplevel's window, if it is mapped.
    fn on_window(
        &mut self,
        toplevel: &ToplevelSurface,
        operation: impl FnOnce(&mut Desktop, WindowId),
    ) {
        if let Some(id) = self.window_id(toplevel.wl_surface()) {
            operation(&mut self.desktop, id);
        }
    }
}

/// A toplevel's application identifier and title, empty where unset.
fn names(surface: &WlSurface) -> (String, String) {
    with_states(surface, |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok())
            .map(|attributes| {
                (
                    attributes.app_id.clone().unwrap_or_default(),
                    attributes.title.clone().unwrap_or_default(),
                )
            })
            .unwrap_or_default()
    })
}

impl XdgShellHandler for Compositor {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        self.unmapped.push(surface);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let surface = surface.wl_surface();
        self.unmapped
            .retain(|unmapped| unmapped.wl_surface() != surface);
        if let Some(id) = self.window_id(surface) {
            self.desktop.remove_window(id);
            self.windows.retain(|mapped| {
                let keep = mapped.id != id;
                if !keep {
                    self.space.unmap_elem(&mapped.window);
                }
                keep
            });
        }
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        let (_, title) = names(surface.wl_surface());
        self.on_window(&surface, |desktop, id| desktop.set_title(id, title));
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        self.on_window(&surface, |desktop, id| desktop.set_maximized(id, true));
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        self.on_window(&surface, |desktop, id| desktop.set_maximized(id, false));
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        self.on_window(&surface, |desktop, id| desktop.set_fullscreen(id, true));
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        self.on_window(&surface, |desktop, id| desktop.set_fullscreen(id, false));
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        self.on_window(&surface, Desktop::minimize);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        surface.with_pending_state(|state| state.geometry = positioner.get_geometry());
        // Tracking only fails for a popup without a parent, which is then
        // never shown.
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // Popup grabs (dismissing a menu by clicking elsewhere) are not
        // implemented yet; the popup still shows and works.
    }
}

impl SeatHandler for Compositor {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seats
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}
}

impl ShmHandler for Compositor {
    fn shm_state(&self) -> &ShmState {
        &self.shm
    }
}

impl BufferHandler for Compositor {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl OutputHandler for Compositor {}

delegate_compositor!(Compositor);
delegate_xdg_shell!(Compositor);
delegate_shm!(Compositor);
delegate_seat!(Compositor);
delegate_output!(Compositor);
