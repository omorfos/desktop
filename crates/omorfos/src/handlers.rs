//! Wayland protocol handlers: what clients ask for, translated into
//! operations on the desktop model.

use omorfos_core::geometry::Size as DomainSize;
use omorfos_core::{Desktop, Edges, WindowId};
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};
use smithay::delegate_dmabuf;
use smithay::backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state};
use smithay::desktop::{LayerSurface, PopupKind, Window, WindowSurfaceType, layer_map_for_output};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::{ResizeEdge, State as ToplevelState};
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
use smithay::wayland::foreign_toplevel_list::{ForeignToplevelListHandler, ForeignToplevelListState};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::ext_data_control::{DataControlHandler, DataControlState};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
use std::os::fd::OwnedFd;
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState, set_primary_focus,
};
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
use smithay::output::Output;
use smithay::wayland::fractional_scale::FractionalScaleHandler;
use smithay::wayland::tablet_manager::TabletSeatHandler;
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::{
    delegate_compositor, delegate_cursor_shape, delegate_data_device, delegate_foreign_toplevel_list, delegate_fractional_scale, delegate_single_pixel_buffer, delegate_viewporter, delegate_layer_shell, delegate_primary_selection, delegate_xdg_activation, delegate_output, delegate_xdg_decoration, delegate_seat, delegate_shm, delegate_xdg_shell,
};

use crate::compositor::{ClientState, Compositor, Mapped, logical_size, set};
use crate::roles::Roles;
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::XWaylandClientData;

use crate::input::Wanted;
use crate::x11::{Focus, Role};

impl CompositorHandler for Compositor {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.surfaces
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        let own = client.get_data::<ClientState>();
        let own = own.map(|state| &state.compositor);
        // The X server is a client that Smithay inserts, with its own state.
        let x11 = || client.get_data::<XWaylandClientData>();
        #[expect(
            clippy::expect_used,
            reason = "every client is inserted with a `ClientState`, but for the X server"
        )]
        let state = own
            .or_else(|| x11().map(|state| &state.compositor_state))
            .expect("client without state");
        state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        if let Some(mapped) = self.windows.iter().find(|mapped| mapped.surface == root) {
            mapped.window.on_commit();
            // A window may resize itself. Believe its size only once it has
            // acknowledged the size we last asked for; before that, a
            // differing size is just the old one still in flight.
            let asked = self
                .desktop
                .frames()
                .find(|frame| frame.window().id == mapped.id)
                .map(|frame| frame.client().size);
            // A program written for X11 asks before it changes size.
            let acknowledged = match &mapped.role {
                Role::Xdg(toplevel) => toplevel.current_state().size.map(domain_size),
                Role::X11(_) => None,
            };
            let actual = domain_size(mapped.window.geometry().size);
            if asked.is_some() && acknowledged == asked && asked != Some(actual) {
                self.desktop.set_client_size(mapped.id, actual);
            }
        }

        self.map_if_ready(surface);
        self.map_x11_if_ready(surface);
        // The pointer an application draws may be what has changed.
        self.pointer_changed(&root);

        // A panel, dock or wallpaper: lay the layers out again and tell it
        // the size it has been given.
        for head in &self.heads {
            let mut layers = layer_map_for_output(&head.output);
            let layer = layers
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .cloned();
            if let Some(layer) = layer {
                layers.arrange();
                layer.layer_surface().send_pending_configure();
            }
        }

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
        let (app_id, title) = names(surface);
        // The window expects us to decorate it if it negotiated that.
        let wants_decoration = self.unmapped[index]
            .with_pending_state(|state| state.decoration_mode == Some(DecorationMode::ServerSide));
        if !self.unmapped[index].is_initial_configure_sent() {
            // A window that rules open maximized or fullscreen is told so
            // now, before it draws, so that it never shows at another size.
            let opening =
                self.desktop
                    .opening(&app_id, &title, self.pointer_position(), wants_decoration);
            self.unmapped[index].with_pending_state(|state| {
                if let Some(opening) = opening {
                    state.size = Some(logical_size(opening.size));
                    set(
                        &mut state.states,
                        ToplevelState::Maximized,
                        opening.maximized,
                    );
                    set(
                        &mut state.states,
                        ToplevelState::Fullscreen,
                        opening.fullscreen,
                    );
                }
            });
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
        let size = domain_size(window.geometry().size);
        let listed = self.foreign_toplevels.new_toplevel::<Self>(&title, &app_id);
        // A dialog says which window it belongs to, and opens where that
        // one is.
        let parent = toplevel.parent();
        let parent = parent.and_then(|parent| self.window_id(&parent));
        let id = self.desktop.add_window_of(
            parent,
            app_id,
            title,
            size,
            self.pointer_position(),
            wants_decoration,
        );
        self.windows.push(Mapped {
            id,
            window,
            surface: surface.clone(),
            role: Role::Xdg(toplevel),
            listed,
        });
        let client = surface.client();
        let credentials = client.and_then(|client| client.get_credentials(&self.display).ok());
        let pid = credentials.and_then(|credentials| u32::try_from(credentials.pid).ok());
        if self.asked_for(surface, pid, None) {
            self.desktop.activate(id);
        }
    }

    /// Takes a window that has gone out of the desktop.
    pub(crate) fn remove_window(&mut self, id: WindowId) {
        self.desktop.remove_window(id);
        self.windows.retain(|mapped| {
            let keep = mapped.id != id;
            if !keep {
                self.space.unmap_elem(&mapped.window);
                self.foreign_toplevels.remove_toplevel(&mapped.listed);
            }
            keep
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
            self.remove_window(id);
        }
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        let (_, title) = names(surface.wl_surface());
        if let Some(mapped) = self
            .windows
            .iter()
            .find(|mapped| mapped.role == Role::Xdg(surface.clone()))
        {
            mapped.listed.send_title(&title);
            mapped.listed.send_done();
            self.desktop.set_title(mapped.id, title);
        }
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

// Applications are told the scale from `Compositor::announce_scale`, for
// every surface alike, so there is nothing to do when one asks to be told.
impl FractionalScaleHandler for Compositor {}

// Naming a shape for the pointer is part of a protocol that also covers
// drawing tablets, which are not supported; this is all it needs of them.
impl TabletSeatHandler for Compositor {}

impl WlrLayerShellHandler for Compositor {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // A surface outside windows is for a program that was started to
        // put one there.
        let client = surface.wl_surface().client();
        let roles = Self::roles(client.as_ref());
        if !roles.has(Roles::PANEL.with(Roles::OVERLAY)) {
            self.refuse(client.as_ref(), "put a panel on the screen", "panel");
            surface.send_close();
            return;
        }
        // On the output it asked for, or else the one the pointer is on.
        let asked = output.as_ref().and_then(Output::from_resource);
        let asked = asked.as_ref().and_then(|output| self.head(output));
        match asked.or_else(|| self.pointer_head()) {
            // Mapping fails only for a surface that is already mapped,
            // which a new one cannot be.
            Some(head) => {
                let layer = LayerSurface::new(surface, namespace);
                let _ = layer_map_for_output(&head.output).map_layer(&layer);
            }
            // With no screen there is nowhere to put it.
            None => surface.send_close(),
        }
    }
}

impl SeatHandler for Compositor {
    type KeyboardFocus = Focus;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seats
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&Focus>) {
        // The clipboard, and the selection pasted with the middle button,
        // are offered to whichever client has the keyboard.
        let surface = focused.and_then(WaylandFocus::wl_surface);
        let client = surface.and_then(|surface| self.display.get_client(surface.id()).ok());
        set_data_device_focus(&self.display, seat, client.clone());
        set_primary_focus(&self.display, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        // Whether it is shown depends on where the pointer is; see
        // `Compositor::cursor_wanted`.
        self.cursor = image;
    }
}

impl DataControlHandler for Compositor {
    fn data_control_state(&self) -> &DataControlState {
        &self.clipboards
    }
}

smithay::delegate_ext_data_control!(Compositor);

impl SelectionHandler for Compositor {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        which: SelectionTarget,
        source: Option<SelectionSource>,
        _: Seat<Self>,
    ) {
        // What a program written for Wayland has copied or selected is
        // there for programs written for X11 as well.
        self.offer_to_x11(which, source.map(|source| source.mime_types()));
    }

    fn send_selection(
        &mut self,
        which: SelectionTarget,
        kind: String,
        to: OwnedFd,
        _: Seat<Self>,
        (): &(),
    ) {
        // The only selections that are ours to send are those of programs
        // written for X11, which the X server is asked for.
        self.send_from_x11(which, kind, to);
    }
}

impl DataDeviceHandler for Compositor {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device
    }
}

impl PrimarySelectionHandler for Compositor {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection
    }
}

impl ForeignToplevelListHandler for Compositor {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.foreign_toplevels
    }
}

impl ClientDndGrabHandler for Compositor {}

impl ServerDndGrabHandler for Compositor {}

impl XdgActivationHandler for Compositor {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.asked.tokens
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        // A token is the right to move focus. It is honoured only if the
        // application tied it to something the user did (a click, a key),
        // so that windows cannot raise themselves unprompted.
        data.serial.is_some()
    }

    fn request_activation(
        &mut self,
        token: XdgActivationToken,
        _data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        // A token says so once.
        self.asked.tokens.remove_token(&token);
        match self.window_id(&surface) {
            Some(id) => self.desktop.activate(id),
            // Before its window is shown, as a program started with a token
            // may activate it: it is shown when it opens.
            None => self.ask_early(surface),
        }
    }
}

impl ShmHandler for Compositor {
    fn shm_state(&self) -> &ShmState {
        &self.shm
    }
}

impl BufferHandler for Compositor {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl DmabufHandler for Compositor {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.gpu_buffers.state
    }

    fn dmabuf_imported(&mut self, _: &DmabufGlobal, _: Dmabuf, notifier: ImportNotifier) {
        // Only what the card can draw is offered, which Smithay has checked
        // the buffer is: whether it can be drawn is found when it is.
        let _ = notifier.successful::<Self>();
    }
}

impl OutputHandler for Compositor {}

delegate_compositor!(Compositor);
delegate_xdg_shell!(Compositor);
delegate_layer_shell!(Compositor);
delegate_data_device!(Compositor);
delegate_foreign_toplevel_list!(Compositor);
delegate_primary_selection!(Compositor);
delegate_xdg_activation!(Compositor);
delegate_xdg_decoration!(Compositor);
delegate_shm!(Compositor);
delegate_seat!(Compositor);
delegate_output!(Compositor);
delegate_fractional_scale!(Compositor);
delegate_cursor_shape!(Compositor);
delegate_viewporter!(Compositor);
delegate_single_pixel_buffer!(Compositor);
delegate_dmabuf!(Compositor);

#[cfg(test)]
mod tests {

    use super::*;
    use crate::Session;

    #[test]
    fn smithay_is_handed_the_compositors_own_clipboard_state() {
        // Smithay requires this accessor but, as of 0.7, never calls it
        // itself, so no client request can exercise it.
        let directory = tempfile::TempDir::new().unwrap();
        let session = Session::new(directory.path()).unwrap();
        let compositor = session.compositor();
        assert!(std::ptr::eq(
            compositor.data_device_state(),
            &raw const compositor.data_device
        ));
    }
}
