//! Wayland protocol handlers: what clients ask for, translated into
//! operations on the desktop model.

use std::borrow::Cow;

use omorfos_core::geometry::Size as DomainSize;
use omorfos_core::{Desktop, Edges, WindowId};
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};
use smithay::delegate_dmabuf;
use smithay::backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state};
use smithay::desktop::{
    LayerSurface, PopupKind, Window, WindowSurfaceType, find_popup_root_surface,
    get_popup_toplevel_coords, layer_map_for_output,
};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::{ResizeEdge, State as ToplevelState};
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Serial, Size};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes, get_parent,
    with_states,
};
use smithay::wayland::foreign_toplevel_list::{ForeignToplevelListHandler, ForeignToplevelListState};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::ext_data_control::{DataControlHandler, DataControlState};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
use std::os::fd::OwnedFd;
use std::sync::Arc;
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
use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::{
    delegate_alpha_modifier, delegate_compositor, delegate_cursor_shape, delegate_data_device, delegate_foreign_toplevel_list, delegate_fractional_scale, delegate_single_pixel_buffer, delegate_viewporter, delegate_layer_shell, delegate_primary_selection, delegate_xdg_activation, delegate_xdg_foreign, delegate_output, delegate_xdg_decoration, delegate_seat, delegate_shm, delegate_xdg_shell,
};

use crate::compositor::{ClientState, Compositor, DragIcon, Mapped, logical_size, set};
use crate::roles::Roles;
use crate::tiles;
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
            // A window may say anew how small it can be, which a part it
            // fills is made room for.
            self.desktop.set_least(mapped.id, tiles::least(mapped));
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
            // A line dragged ahead of the windows goes on once they have
            // drawn themselves.
            self.catch_line_up();
        }

        self.map_if_ready(surface);
        self.map_x11_if_ready(surface);
        // A picture of what is dragged may be moved as it is given, to hang
        // from the pointer elsewhere.
        let icon = self.hover.dragged.as_mut();
        if let Some(icon) = icon.filter(|icon| icon.surface == *surface) {
            let moved = with_states(surface, |states| {
                let mut attributes = states.cached_state.get::<SurfaceAttributes>();
                attributes.current().buffer_delta.take()
            });
            icon.offset += moved.unwrap_or_default();
        }
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
        // One kept through a change of compositor comes back where it was.
        let kept_by = crate::kept::key(&toplevel);
        crate::kept::listed(self.autostarted.parts.relay.as_ref(), &listed, kept_by);
        let id = match self.kept_window(kept_by) {
            Some(kept) => self
                .desktop
                .add_window_kept(app_id, title, wants_decoration, &kept),
            None => self.desktop.add_window_of(
                parent,
                app_id,
                title,
                size,
                self.pointer_position(),
                wants_decoration,
            ),
        };
        self.windows.push(Mapped {
            id,
            window,
            surface: surface.clone(),
            role: Role::Xdg(toplevel),
            listed,
            kept_by,
        });
        // How small it said it can be before it was shown.
        let least = self.windows.last().map(tiles::least).unwrap_or_default();
        self.desktop.set_least(id, least);
        let client = surface.client();
        let pid = client.and_then(|client| self.process(&client));
        if self.asked_for(surface, pid, None) {
            self.desktop.activate(id);
        }
    }

    /// Takes a window that has gone out of the desktop.
    pub(crate) fn remove_window(&mut self, id: WindowId) {
        self.desktop.remove_window(id);
        self.forget_use(id);
        let relay = self.autostarted.parts.relay.as_ref();
        self.windows.retain(|mapped| {
            let keep = mapped.id != id;
            if !keep {
                self.space.unmap_elem(&mapped.window);
                self.foreign_toplevels.remove_toplevel(&mapped.listed);
                crate::kept::unlisted(relay, &mapped.listed);
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
        let geometry = self.popup_placed(&surface, positioner);
        surface.with_pending_state(|state| state.geometry = geometry);
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
        let geometry = self.popup_placed(&surface, positioner);
        surface.with_pending_state(|state| {
            state.geometry = geometry;
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
    /// Where a popup goes: where its positioner says, kept on the screen
    /// what it hangs from is on, as far as the positioner lets it be
    /// flipped or slid, as a bar's menu along the bottom is turned to hang
    /// above it. As the positioner says, where what it hangs from is not
    /// on a screen.
    fn popup_placed(
        &self,
        popup: &PopupSurface,
        positioner: PositionerState,
    ) -> Rectangle<i32, Logical> {
        let kind = PopupKind::Xdg(popup.clone());
        // Popups are placed against what they hang from, and that, for one
        // that hangs from another, by where that one is in theirs.
        let root = find_popup_root_surface(&kind).ok();
        let origin = root.and_then(|root| self.geometry_origin(&root));
        let base = origin.map(|origin| origin + get_popup_toplevel_coords(&kind));
        let anchor = positioner.anchor_rect.loc;
        let screen = base.and_then(|base| {
            let head = self.head_at((base + anchor).to_f64());
            head.map(|head| Rectangle::new(head.area().loc - base, head.area().size))
        });
        let as_said = positioner.get_geometry();
        screen.map_or(as_said, |screen| {
            positioner.get_unconstrained_geometry(screen)
        })
    }

    /// Where the window geometry of `surface`, the main surface of a window
    /// or of a panel that is shown, begins on the screens: what its popups
    /// are placed against. A panel's is its surface.
    fn geometry_origin(&self, surface: &WlSurface) -> Option<Point<i32, Logical>> {
        let window = self.scene.iter().find(|shown| shown.surface == *surface);
        let window = window.map(|shown| shown.origin);
        window.or_else(|| self.surface_origin(surface))
    }

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

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        // A panel's popup is made without a parent and given one only now,
        // before it is first shown: only now can it be kept on the screen.
        let positioner = popup.with_pending_state(|state| state.positioner);
        let geometry = self.popup_placed(&popup, positioner);
        popup.with_pending_state(|state| state.geometry = geometry);
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
        if !roles.has(Roles::PANEL.with(Roles::OVERLAY).with(Roles::ASK)) {
            self.refuse(client.as_ref(), "panel", "panel");
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
        let client = surface
            .as_ref()
            .and_then(|surface| self.display.get_client(surface.id()).ok());
        set_data_device_focus(&self.display, seat, client.clone());
        set_primary_focus(&self.display, seat, client);
        // A program holds the pointer only while its window has the keyboard.
        self.protocols.holding.keyboard = surface.map(Cow::into_owned);
        self.hold();
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

/// What the compositor offers on the clipboard as its own.
#[derive(Debug, Clone)]
pub enum Offer {
    /// What a program written for X11 has copied or selected, which the X
    /// server is asked for.
    X11,
    /// Text of the compositor's own, as "About" copies what the computer is.
    Text(Arc<str>),
}

/// The forms text of the compositor's own is offered in: those programs
/// written for Wayland ask for, then those of X11's.
pub(crate) const TEXT: [&str; 5] = [
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "TEXT",
    "STRING",
];

impl Compositor {
    /// Hands `text` of the compositor's own to a program that pastes it,
    /// through `to`, the pipe the program gave for it, without waiting on
    /// the program (ADR 90): one that does not read, or gave no pipe, gets
    /// nothing, as one that has gone does.
    pub(crate) fn hand_over(&mut self, text: &Arc<str>, to: OwnedFd) {
        let _ = self.handing.hand(Arc::from(Arc::clone(text)), to);
    }

    /// How many programs are being handed text of the compositor's own
    /// just now, each on a thread of its own: for tests, which wait for it.
    pub fn handing_over(&mut self) -> usize {
        self.handing.busy()
    }
}

impl SelectionHandler for Compositor {
    type SelectionUserData = Offer;

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
        offer: &Offer,
    ) {
        // What a program written for X11 holds, the X server is asked for;
        // what is the compositor's own, it writes.
        match offer {
            Offer::X11 => self.send_from_x11(which, kind, to),
            Offer::Text(text) => self.hand_over(text, to),
        }
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

impl ClientDndGrabHandler for Compositor {
    fn started(&mut self, _: Option<WlDataSource>, icon: Option<WlSurface>, _: Seat<Self>) {
        let offset = Point::default();
        self.hover.dragged = icon.map(|surface| DragIcon { surface, offset });
    }

    fn dropped(&mut self, _: Option<WlSurface>, _: bool, _: Seat<Self>) {
        self.hover.dragged = None;
    }
}

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

impl XdgForeignHandler for Compositor {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.protocols.foreign
    }
}
delegate_xdg_foreign!(Compositor);
delegate_xdg_decoration!(Compositor);
delegate_shm!(Compositor);
delegate_seat!(Compositor);
delegate_output!(Compositor);
delegate_fractional_scale!(Compositor);
delegate_cursor_shape!(Compositor);
delegate_viewporter!(Compositor);
delegate_single_pixel_buffer!(Compositor);
delegate_alpha_modifier!(Compositor);
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
