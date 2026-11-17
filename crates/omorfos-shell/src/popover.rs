//! A popover: a small window that hangs from a component's surface on the
//! screen, as the month does from the panel's clock.
//!
//! The compositor places it, under what was clicked where it fits on the
//! screen, slid or turned over where it does not, and takes it away at a
//! click anywhere else, which it knows by the popover's taking the click
//! that opened it (an `xdg_popup` and its grab). A component keeps a
//! [`Popovers`], hands it the events of the popover's surface and popup
//! with `delegate_dispatch!`, and paints what is in it. The positioner
//! says nothing, and what makes popovers says nothing but to be pinged,
//! which omorfos does not do to its components: both are left to
//! `delegate_noop!`.
//!
//! A tooltip hangs from the surface the same way, beside the popover that
//! may be open, but takes no click: it is only shown, and the pointer goes
//! through it to what is under it. [`crate::tooltip`] says when.

use std::io;

use omorfos_ui::{Area, Canvas};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_region, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::shell::client::{xdg_popup, xdg_positioner, xdg_surface, xdg_wm_base};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

use crate::sheet::Sheet;

/// What a component keeps of its popovers: what makes them, once the
/// compositor has offered it, the one that is open, and the tooltip shown.
#[derive(Debug, Default)]
pub struct Popovers {
    shell: Option<xdg_wm_base::XdgWmBase>,
    open: Option<Popover>,
    tip: Option<Popover>,
    /// Whether the one open or the tooltip, or their going, is yet to be
    /// shown.
    changed: bool,
}

/// A popover on the screen.
#[derive(Debug)]
struct Popover {
    sheet: Sheet,
    xdg: xdg_surface::XdgSurface,
    popup: xdg_popup::XdgPopup,
    /// How large it is, in units of layout.
    size: (u32, u32),
    /// What it hangs below, on the surface it hangs from.
    under: Area,
    /// Whether the compositor has said it may be drawn on.
    ready: bool,
}

/// Where a tooltip hangs from, and how large it is.
#[derive(Debug, Clone, Copy)]
pub struct Tip<'a> {
    /// The component's surface it hangs from.
    pub from: &'a zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    /// What it is the tooltip of, on that surface, in its units of layout:
    /// it hangs below it, as a popover does.
    pub under: Area,
    /// How large it is, in units of layout.
    pub size: (u32, u32),
}

/// Where a popover hangs from, how large it is, and the click it takes.
#[derive(Debug, Clone, Copy)]
pub struct Hanging<'a> {
    /// The component's surface it hangs from.
    pub from: &'a zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    /// What was clicked, on that surface, in its units of layout: the
    /// popover hangs below it, its middle under that one's.
    pub under: Area,
    /// How large the popover is, in units of layout.
    pub size: (u32, u32),
    /// The seat the click came from, and the click's serial.
    pub click: (&'a wl_seat::WlSeat, u32),
}

impl Popovers {
    /// Takes what makes popovers, which the compositor offers as
    /// `xdg_wm_base`.
    pub fn offered(&mut self, shell: xdg_wm_base::XdgWmBase) {
        self.shell = Some(shell);
    }

    /// Opens a popover hanging as `hanging` says, in place of any open,
    /// with the compositor's `compositor` and `viewporter`. Says whether
    /// it could be: not before the compositor has offered what makes them.
    pub fn open<S>(
        &mut self,
        (compositor, viewporter): (&wl_compositor::WlCompositor, &wp_viewporter::WpViewporter),
        hanging: Hanging<'_>,
        handle: &QueueHandle<S>,
    ) -> bool
    where
        S: Dispatch<wl_surface::WlSurface, ()>
            + Dispatch<wp_viewport::WpViewport, ()>
            + Dispatch<xdg_surface::XdgSurface, ()>
            + Dispatch<xdg_popup::XdgPopup, ()>
            + Dispatch<xdg_positioner::XdgPositioner, ()>
            + 'static,
    {
        self.close();
        // What it hangs from is taken up by what is opened.
        self.untip();
        let made = self.shell.as_ref().map(|shell| {
            let tip = Tip {
                from: hanging.from,
                under: hanging.under,
                size: hanging.size,
            };
            Popover::made(shell, (compositor, viewporter), tip, handle)
        });
        let opened = made.inspect(|popover| {
            let (seat, serial) = hanging.click;
            popover.popup.grab(seat, serial);
            // The compositor answers this by saying it may be drawn on.
            popover.sheet.surface().commit();
        });
        self.changed |= opened.is_some();
        self.open = opened;
        self.open.is_some()
    }

    /// Shows a tooltip hanging as `tip` says, in place of any shown, with
    /// the compositor's `compositor` and `viewporter`. Says whether it
    /// could be: not before the compositor has offered what makes it.
    pub fn tip<S>(
        &mut self,
        (compositor, viewporter): (&wl_compositor::WlCompositor, &wp_viewporter::WpViewporter),
        tip: Tip<'_>,
        handle: &QueueHandle<S>,
    ) -> bool
    where
        S: Dispatch<wl_surface::WlSurface, ()>
            + Dispatch<wl_region::WlRegion, ()>
            + Dispatch<wp_viewport::WpViewport, ()>
            + Dispatch<xdg_surface::XdgSurface, ()>
            + Dispatch<xdg_popup::XdgPopup, ()>
            + Dispatch<xdg_positioner::XdgPositioner, ()>
            + 'static,
    {
        self.untip();
        let shown = self.shell.as_ref().map(|shell| {
            let popover = Popover::made(shell, (compositor, viewporter), tip, handle);
            // Nowhere on it takes the pointer, which goes on to what is
            // under it as if it were not there.
            let nowhere = compositor.create_region(handle, ());
            popover.sheet.surface().set_input_region(Some(&nowhere));
            nowhere.destroy();
            popover.sheet.surface().commit();
            popover
        });
        self.changed |= shown.is_some();
        self.tip = shown;
        self.tip.is_some()
    }

    /// Takes away the tooltip shown, if one is.
    pub fn untip(&mut self) {
        let tip = self.tip.take();
        self.changed |= tip.is_some();
        tip.into_iter().for_each(Popover::destroy);
    }

    /// Has the popover that is open, if one is, hang below `under`, if it
    /// has moved: as the clock does along a bar that is made narrower.
    /// Nothing changes for nothing to hang below.
    pub fn hang<S>(&mut self, under: Option<Area>, handle: &QueueHandle<S>)
    where
        S: Dispatch<xdg_positioner::XdgPositioner, ()> + 'static,
    {
        let open = self.open.as_mut().zip(under);
        let open = open.filter(|(open, under)| open.under != *under);
        let shell = self.shell.as_ref();
        if let Some(((open, under), shell)) = open.zip(shell) {
            open.under = under;
            let positioner = placed(shell, under, open.size, handle);
            // The compositor answers with where it now is; what is drawn
            // on it stays.
            open.popup.reposition(&positioner, 0);
            positioner.destroy();
        }
    }

    /// Takes away the popover that is open, if one is.
    pub fn close(&mut self) {
        let open = self.open.take();
        self.changed |= open.is_some();
        open.into_iter().for_each(Popover::destroy);
    }

    /// Whether a popover is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Whether `surface` is the popover's.
    #[must_use]
    pub fn is(&self, surface: &wl_surface::WlSurface) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| open.sheet.surface() == surface)
    }

    /// Whether the popover, or its going, is yet to be shown; asked, it is
    /// taken to have been.
    pub fn changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Puts `canvas` on the popover that is open, once the compositor has
    /// said it may be drawn on.
    ///
    /// # Errors
    ///
    /// Returns an error if the picture cannot be handed over.
    pub fn draw<S>(
        &mut self,
        shm: &wl_shm::WlShm,
        canvas: &Canvas,
        handle: &QueueHandle<S>,
    ) -> io::Result<()>
    where
        S: Dispatch<wl_buffer::WlBuffer, ()> + Dispatch<wl_shm_pool::WlShmPool, ()> + 'static,
    {
        let Some(open) = self.open.as_mut().filter(|open| open.ready) else {
            return Ok(());
        };
        open.sheet.put(shm, canvas, open.size, handle)
    }

    /// Puts `canvas` on the tooltip shown, once the compositor has said it
    /// may be drawn on.
    ///
    /// # Errors
    ///
    /// Returns an error if the picture cannot be handed over.
    pub fn draw_tip<S>(
        &mut self,
        shm: &wl_shm::WlShm,
        canvas: &Canvas,
        handle: &QueueHandle<S>,
    ) -> io::Result<()>
    where
        S: Dispatch<wl_buffer::WlBuffer, ()> + Dispatch<wl_shm_pool::WlShmPool, ()> + 'static,
    {
        let Some(tip) = self.tip.as_mut().filter(|tip| tip.ready) else {
            return Ok(());
        };
        tip.sheet.put(shm, canvas, tip.size, handle)
    }
}

impl Popover {
    /// Makes a popover's surface and popup, hanging from and as large as
    /// `tip` says, neither yet committed.
    fn made<S>(
        shell: &xdg_wm_base::XdgWmBase,
        (compositor, viewporter): (&wl_compositor::WlCompositor, &wp_viewporter::WpViewporter),
        tip: Tip<'_>,
        handle: &QueueHandle<S>,
    ) -> Self
    where
        S: Dispatch<wl_surface::WlSurface, ()>
            + Dispatch<wp_viewport::WpViewport, ()>
            + Dispatch<xdg_surface::XdgSurface, ()>
            + Dispatch<xdg_popup::XdgPopup, ()>
            + Dispatch<xdg_positioner::XdgPositioner, ()>
            + 'static,
    {
        let surface = compositor.create_surface(handle, ());
        let viewport = viewporter.get_viewport(&surface, handle, ());
        let xdg = shell.get_xdg_surface(&surface, handle, ());
        let positioner = placed(shell, tip.under, tip.size, handle);
        let popup = xdg.get_popup(None, &positioner, handle, ());
        tip.from.get_popup(&popup);
        positioner.destroy();
        Self {
            sheet: Sheet::new(surface, viewport),
            xdg,
            popup,
            size: tip.size,
            under: tip.under,
            ready: false,
        }
    }

    /// Destroys its objects, the popup first, as the protocol has it.
    fn destroy(self) {
        self.popup.destroy();
        self.xdg.destroy();
        self.sheet.destroy();
    }
}

/// Where a popover `size` large goes: below `under`, its middle under that
/// one's, and kept on the screen, slid along it, or turned over to hang
/// above `under`, as for a bar along the bottom.
fn placed<S>(
    shell: &xdg_wm_base::XdgWmBase,
    under: Area,
    (width, height): (u32, u32),
    handle: &QueueHandle<S>,
) -> xdg_positioner::XdgPositioner
where
    S: Dispatch<xdg_positioner::XdgPositioner, ()> + 'static,
{
    let (left, top, wide, tall) = under;
    let positioner = shell.create_positioner(handle, ());
    positioner.set_size(width.cast_signed(), height.cast_signed());
    positioner.set_anchor_rect(left, top, wide.cast_signed(), tall.cast_signed());
    positioner.set_anchor(xdg_positioner::Anchor::Bottom);
    positioner.set_gravity(xdg_positioner::Gravity::Bottom);
    let kept = xdg_positioner::ConstraintAdjustment::SlideX
        | xdg_positioner::ConstraintAdjustment::FlipY
        | xdg_positioner::ConstraintAdjustment::SlideY;
    positioner.set_constraint_adjustment(kept);
    positioner
}

impl<S> Dispatch<xdg_surface::XdgSurface, (), S> for Popovers
where
    S: Dispatch<xdg_surface::XdgSurface, ()> + AsMut<Self>,
{
    fn event(
        state: &mut S,
        xdg: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
        // All it says is that the popover is placed: the first time, that
        // it may be drawn on; again, once it has been placed anew, with
        // what was drawn on it.
        if let xdg_surface::Event::Configure { serial } = event
            && first(state.as_mut(), xdg, serial)
        {
            state.as_mut().changed = true;
        }
    }
}

impl<S> Dispatch<xdg_popup::XdgPopup, (), S> for Popovers
where
    S: Dispatch<xdg_popup::XdgPopup, ()> + AsMut<Self>,
{
    fn event(
        state: &mut S,
        popup: &xdg_popup::XdgPopup,
        event: xdg_popup::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
        // The compositor took it away, at a click elsewhere; where it is
        // placed is said again on its surface, and its size is its own.
        if matches!(event, xdg_popup::Event::PopupDone) {
            let popovers = state.as_mut();
            let slots = [&mut popovers.open, &mut popovers.tip].into_iter();
            let done: Vec<Popover> = slots
                .filter_map(|slot| slot.take_if(|open| open.popup == *popup))
                .collect();
            popovers.changed |= !done.is_empty();
            done.into_iter().for_each(Popover::destroy);
        }
    }
}

/// Answers the compositor's placing of `xdg` at `serial`, and says whether
/// it is the first time, after which its popover may be drawn on. Placed
/// anew, it moves once it has said so, with what is drawn on it.
fn first(popovers: &mut Popovers, xdg: &xdg_surface::XdgSurface, serial: u32) -> bool {
    xdg.ack_configure(serial);
    let mut slots = popovers.open.iter_mut().chain(popovers.tip.iter_mut());
    let open = slots.find(|open| open.xdg == *xdg);
    open.is_some_and(|open| {
        let first = !std::mem::replace(&mut open.ready, true);
        (!first).then(|| open.sheet.surface().commit());
        first
    })
}
