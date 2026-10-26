//! What the relay knows of each protocol in the list: the description of
//! every message, by which it reads them.

use wayland_backend::protocol::Interface;
use wayland_server::Resource;
use wayland_server::protocol::{
    wl_compositor, wl_data_device_manager, wl_output, wl_seat, wl_shm, wl_subcompositor,
};

use wayland_protocols::ext::data_control::v1::server::ext_data_control_manager_v1;
use wayland_protocols::ext::foreign_toplevel_list::v1::server::ext_foreign_toplevel_list_v1;
use wayland_protocols::ext::idle_notify::v1::server::ext_idle_notifier_v1;
use wayland_protocols::ext::image_capture_source::v1::server::{
    ext_foreign_toplevel_image_capture_source_manager_v1,
    ext_output_image_capture_source_manager_v1,
};
use wayland_protocols::ext::image_copy_capture::v1::server::ext_image_copy_capture_manager_v1;
use wayland_protocols::ext::session_lock::v1::server::ext_session_lock_manager_v1;
use wayland_protocols::wp::alpha_modifier::v1::server::wp_alpha_modifier_v1;
use wayland_protocols::wp::cursor_shape::v1::server::wp_cursor_shape_manager_v1;
use wayland_protocols::wp::fractional_scale::v1::server::wp_fractional_scale_manager_v1;
use wayland_protocols::wp::idle_inhibit::zv1::server::zwp_idle_inhibit_manager_v1;
use wayland_protocols::wp::linux_dmabuf::zv1::server::zwp_linux_dmabuf_v1;
use wayland_protocols::wp::pointer_constraints::zv1::server::zwp_pointer_constraints_v1;
use wayland_protocols::wp::pointer_gestures::zv1::server::zwp_pointer_gestures_v1;
use wayland_protocols::wp::presentation_time::server::wp_presentation;
use wayland_protocols::wp::primary_selection::zv1::server::zwp_primary_selection_device_manager_v1;
use wayland_protocols::wp::relative_pointer::zv1::server::zwp_relative_pointer_manager_v1;
use wayland_protocols::wp::security_context::v1::server::wp_security_context_manager_v1;
use wayland_protocols::wp::single_pixel_buffer::v1::server::wp_single_pixel_buffer_manager_v1;
use wayland_protocols::wp::text_input::zv3::server::zwp_text_input_manager_v3;
use wayland_protocols::wp::viewporter::server::wp_viewporter;
use wayland_protocols::xdg::activation::v1::server::xdg_activation_v1;
use wayland_protocols::xdg::decoration::zv1::server::zxdg_decoration_manager_v1;
use wayland_protocols::xdg::foreign::zv2::server::{zxdg_exporter_v2, zxdg_importer_v2};
use wayland_protocols::xdg::shell::server::xdg_wm_base;
use wayland_protocols::xdg::toplevel_icon::v1::server::xdg_toplevel_icon_manager_v1;
use wayland_protocols::xdg::xdg_output::zv1::server::zxdg_output_manager_v1;
use wayland_protocols::xwayland::shell::v1::server::xwayland_shell_v1;
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_manager_v2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_manager_v1;
use wayland_protocols_wlr::layer_shell::v1::server::zwlr_layer_shell_v1;

/// The object every connection starts with.
pub(crate) fn display() -> &'static Interface {
    // A server holds no display object, so its description is only among
    // the generated ones, which the crate keeps out of its documentation.
    &wayland_server::protocol::__interfaces::WL_DISPLAY_INTERFACE
}

/// The global named `name`, if the compositor offers it: the relay reads
/// no other.
pub(crate) fn global(name: &str) -> Option<&'static Interface> {
    GLOBALS
        .iter()
        .map(|interface| interface())
        .find(|interface| interface.name == name)
}

/// Every global the compositor offers, as `omorfos-ipc`'s list says.
const GLOBALS: [fn() -> &'static Interface; 38] = [
    ext_data_control_manager_v1::ExtDataControlManagerV1::interface,
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1::interface,
    ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1::interface,
    ext_idle_notifier_v1::ExtIdleNotifierV1::interface,
    ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1::interface,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1::interface,
    ext_session_lock_manager_v1::ExtSessionLockManagerV1::interface,
    wl_compositor::WlCompositor::interface,
    wl_data_device_manager::WlDataDeviceManager::interface,
    wl_output::WlOutput::interface,
    wl_seat::WlSeat::interface,
    wl_shm::WlShm::interface,
    wl_subcompositor::WlSubcompositor::interface,
    wp_alpha_modifier_v1::WpAlphaModifierV1::interface,
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1::interface,
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1::interface,
    wp_presentation::WpPresentation::interface,
    wp_security_context_manager_v1::WpSecurityContextManagerV1::interface,
    wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1::interface,
    wp_viewporter::WpViewporter::interface,
    xdg_activation_v1::XdgActivationV1::interface,
    xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1::interface,
    xdg_wm_base::XdgWmBase::interface,
    xwayland_shell_v1::XwaylandShellV1::interface,
    zwlr_layer_shell_v1::ZwlrLayerShellV1::interface,
    zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1::interface,
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2::interface,
    zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1::interface,
    zwp_pointer_constraints_v1::ZwpPointerConstraintsV1::interface,
    zwp_pointer_gestures_v1::ZwpPointerGesturesV1::interface,
    zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1::interface,
    zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1::interface,
    zwp_text_input_manager_v3::ZwpTextInputManagerV3::interface,
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1::interface,
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1::interface,
    zxdg_exporter_v2::ZxdgExporterV2::interface,
    zxdg_importer_v2::ZxdgImporterV2::interface,
    zxdg_output_manager_v1::ZxdgOutputManagerV1::interface,
];

#[cfg(test)]
mod tests {
    use omorfos_ipc::protocols::OFFERED;

    use super::*;

    #[test]
    fn each_protocol_offered_is_known_at_least_at_the_version_offered() {
        for protocol in OFFERED {
            let known = global(protocol.interface).map(|interface| interface.version);
            assert!(
                known >= Some(protocol.version),
                "{} at {}",
                protocol.interface,
                protocol.version
            );
        }
    }

    #[test]
    fn only_the_protocols_offered_are_known() {
        assert_eq!(GLOBALS.len(), OFFERED.len());
        assert!(global("wl_drm").is_none());
    }

    #[test]
    fn every_connection_starts_with_the_display() {
        assert_eq!(display().name, "wl_display");
    }
}
