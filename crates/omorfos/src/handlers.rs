//! Wayland protocol handlers: what clients ask for, translated into
//! operations on the desktop model.

use omorfos_core::geometry::Size as DomainSize;
use omorfos_core::{Desktop, Edges, WindowId};
use smithay::backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state};
use smithay::desktop::{LayerSurface, PopupKind, Window, WindowSurfaceType, layer_map_for_output};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Serial, Size};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, get_parent, with_states,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler, set_data_device_focus,
};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::wayland::shell::wlr_layer::{Layer, LayerSurface as WlrLayerSurface, WlrLayerShellHandler, WlrLayerShellState};
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_layer_shell, delegate_output, delegate_xdg_decoration, delegate_seat, delegate_shm, delegate_xdg_shell,
};

use crate::compositor::{ClientState, Compositor, Mapped};
use crate::input::Wanted;

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
        if let Some(mapped) = self
            .windows
            .iter()
            .find(|mapped| *mapped.toplevel.wl_surface() == root)
        {
            mapped.window.on_commit();
            // A window may resize itself. Believe its size only once it has
            // acknowledged the size we last asked for; before that, a
            // differing size is just the old one still in flight.
            let asked = self
                .desktop
                .frames()
                .find(|frame| frame.window().id == mapped.id)
                .map(|frame| frame.client().size);
            let acknowledged = mapped.toplevel.current_state().size.map(domain_size);
            let actual = domain_size(mapped.window.geometry().size);
            if asked.is_some() && acknowledged == asked && asked != Some(actual) {
                self.desktop.set_client_size(mapped.id, actual);
            }
        }

        self.map_if_ready(surface);

        // A panel, dock or wallpaper: lay the layers out again and tell it
        // the size it has been given.
        let mut layers = layer_map_for_output(self.output());
        let layer = layers
            .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .cloned();
        if let Some(layer) = layer {
            layers.arrange();
            layer.layer_surface().send_pending_configure();
        }
        drop(layers);

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
        let size = domain_size(window.geometry().size);
        // The window expects us to decorate it if it negotiated that.
        let wants_decoration = toplevel
            .with_pending_state(|state| state.decoration_mode == Some(DecorationMode::ServerSide));
        let id = self.desktop.add_window(
            app_id,
            title,
            size,
            self.pointer_position(),
            wants_decoration,
        );
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

fn domain_size(size: Size<i32, Logical>) -> DomainSize {
    DomainSize::new(size.w.unsigned_abs(), size.h.unsigned_abs())
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

    fn move_request(&mut self, surface: ToplevelSurface, _seat: WlSeat, _serial: Serial) {
        if let Some(id) = self.window_id(surface.wl_surface()) {
            self.begin(id, Wanted::Move);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: WlSeat,
        _serial: Serial,
        edges: ResizeEdge,
    ) {
        // The protocol encodes the edges as bits: top, bottom, left, right.
        let bits = u32::from(edges);
        let edges = Edges::NONE
            .with_if(bits & 1 != 0, Edges::TOP)
            .with_if(bits & 2 != 0, Edges::BOTTOM)
            .with_if(bits & 4 != 0, Edges::LEFT)
            .with_if(bits & 8 != 0, Edges::RIGHT);
        if let Some(id) = self.window_id(surface.wl_surface()) {
            self.begin(id, Wanted::Resize(edges));
        }
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

    fn grab(&mut self, surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // A menu asks to be told when the user clicks away from it, and to
        // have the keyboard while it is up.
        self.grabbed_popups.push(surface);
    }
}

impl Compositor {
    /// Tells a toplevel that the compositor draws its decoration. Whatever
    /// the client prefers, the answer is the same: frames carry the tabs, so
    /// they are ours to draw. Clients that cannot go without their own
    /// titlebar never ask, and are left undecorated.
    fn decorate(toplevel: &ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        if toplevel.is_initial_configure_sent() {
            toplevel.send_configure();
        }
    }
}

impl XdgDecorationHandler for Compositor {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        Self::decorate(&toplevel);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        Self::decorate(&toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        Self::decorate(&toplevel);
    }
}

impl WlrLayerShellHandler for Compositor {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        _output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // Mapping fails only for a surface that is already mapped, which a
        // new one cannot be.
        let _ =
            layer_map_for_output(self.output()).map_layer(&LayerSurface::new(surface, namespace));
    }
}

impl SeatHandler for Compositor {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seats
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        // The clipboard is offered to whichever client has the keyboard.
        let client = focused.and_then(|surface| self.display.get_client(surface.id()).ok());
        set_data_device_focus(&self.display, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}
}

impl SelectionHandler for Compositor {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Compositor {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device
    }
}

impl ClientDndGrabHandler for Compositor {}

impl ServerDndGrabHandler for Compositor {}

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
delegate_layer_shell!(Compositor);
delegate_data_device!(Compositor);
delegate_xdg_decoration!(Compositor);
delegate_shm!(Compositor);
delegate_seat!(Compositor);
delegate_output!(Compositor);

#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;
    use smithay::utils::Transform;

    use super::*;

    #[test]
    fn smithay_is_handed_the_compositors_own_clipboard_state() {
        // Smithay requires this accessor but, as of 0.7, never calls it
        // itself, so no client request can exercise it.
        let directory = tempfile::TempDir::new().unwrap();
        let display = Display::<Compositor>::new().unwrap();
        let compositor = Compositor::new(
            display.handle(),
            directory.path(),
            (640, 480),
            Transform::Normal,
        )
        .unwrap();
        assert!(std::ptr::eq(
            compositor.data_device_state(),
            &raw const compositor.data_device
        ));
    }
}
