//! New versions of the programs that come with omorfos, taken up as they
//! are installed, without logging in again.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omorfos::Roles;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{LayerShape, Session};
use crate::windows::GREEN;

/// Installs at `path`, as installing does, by putting a new file in the
/// place of the old, a program that stands in for one that runs for the
/// whole session. Started, it writes `version` to the file `marks` beside
/// the session's socket; asked to end, it writes `e` and ends; and it ends
/// as the compositor does, when what it was given to speak to it through
/// closes. That is kept apart first, as the shell gives what it runs in
/// the background nothing to read.
fn install(path: &Path, marks: &str, version: &str) {
    let script = format!(
        "#!/bin/sh\n\
         marks=\"$(dirname \"$OMORFOS_SOCKET\")/{marks}\"\n\
         printf '%s' '{version}' >> \"$marks\"\n\
         trap 'kill $!; printf e >> \"$marks\"; exit 0' TERM\n\
         exec 3<&0\n\
         cat <&3 >/dev/null &\n\
         wait\n"
    );
    written(path, &script);
}

/// Puts `script` at `path`, as a new file in the place of any old one.
fn written(path: &Path, script: &str) {
    let new = path.with_extension("new");
    fs::write(&new, script).unwrap();
    fs::set_permissions(&new, fs::Permissions::from_mode(0o755)).unwrap();
    fs::rename(&new, path).unwrap();
}

/// A session listening where the stand-ins leave their marks, and what
/// reads the marks left in `name`.
fn session(config: &[(&str, &str)]) -> (Session, impl Fn(&str) -> String + use<>) {
    let mut session = Session::with_config(config);
    session.listen();
    let directory = session.config.path().to_owned();
    let marks = move |name: &str| fs::read_to_string(directory.join(name)).unwrap_or_default();
    (session, marks)
}

/// A directory for programs, and where in it the one called `name` is.
fn place(name: &str) -> (tempfile::TempDir, PathBuf) {
    let place = tempfile::TempDir::new().unwrap();
    let path = place.path().join(name);
    (place, path)
}

#[test]
fn a_new_version_of_the_panel_is_started_once_the_one_before_has_ended() {
    let (mut session, marks) = session(&[]);
    let (_place, panel) = place("omorfos-panel");
    install(&panel, "panel", "1");
    session.compositor_mut().set_panel(&panel);
    session.compositor_mut().autostart();
    session.eventually(|_| marks("panel") == "1");
    install(&panel, "panel", "2");
    // The one before is asked to end, and the new one started after it.
    session.eventually(|_| marks("panel") == "1e2");
    session.advance(Duration::from_secs(5));
    assert_eq!(marks("panel"), "1e2", "and that is all");
    assert_eq!(session.compositor().problems(), [] as [&str; 0]);
}

#[test]
fn a_new_keeper_takes_over_before_the_one_before_is_ended() {
    let (mut session, marks) = session(&[]);
    let (_place, keeper) = place("omorfos-clipboard");
    install(&keeper, "keeper", "1");
    session.compositor_mut().set_keeper(&keeper);
    session.compositor_mut().autostart();
    session.eventually(|_| marks("keeper") == "1");
    install(&keeper, "keeper", "2");
    session.eventually(|_| marks("keeper") == "12");
    // A while later, for the new one to have taken what the old keeps.
    session.advance(Duration::from_secs(2));
    assert_eq!(marks("keeper"), "12");
    session.eventually(|_| marks("keeper") == "12e");
}

#[test]
fn what_shares_a_screen_or_asks_for_a_secret_is_left_until_it_no_longer_does() {
    let (mut session, marks) = session(&[]);
    let (_place, portal) = place("xdg-desktop-portal-omorfos");
    let asker = portal.with_file_name("omorfos-ask");
    install(&portal, "portal", "1");
    install(&asker, "asker", "1");
    session.compositor_mut().set_portal(&portal);
    session.compositor_mut().set_asker(&asker);
    session.compositor_mut().autostart();
    session.eventually(|_| marks("portal") == "1" && marks("asker") == "1");

    // A screen shared, and a secret asked for.
    let mut camera = session.connect_as(Roles::CAPTURE);
    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let mut prompt = session.connect_as(Roles::ASK);
    let shape = LayerShape {
        layer: Layer::Overlay,
        anchor: Anchor::empty(),
        size: (100, 50),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::Exclusive,
    };
    let (surface, role) = session.open_layer(&mut prompt, &shape, (100, 50), GREEN);
    install(&portal, "portal", "2");
    install(&asker, "asker", "2");
    session.advance(Duration::from_secs(3));
    session.advance(Duration::from_secs(3));
    assert_eq!((marks("portal"), marks("asker")), ("1".into(), "1".into()));

    // Once the sharing stops, the portal's is taken up; once the prompt
    // goes, the agent's.
    taking.destroy();
    camera.finish_capturing();
    session.roundtrip(&mut camera);
    session.eventually(|_| marks("portal") == "1e2");
    assert_eq!(marks("asker"), "1");
    role.destroy();
    surface.destroy();
    session.roundtrip(&mut prompt);
    session.eventually(|_| marks("asker") == "1e2");
}

#[test]
fn one_given_up_on_is_started_again_by_its_new_version_unless_turned_off() {
    let off = "wallpaper: { enabled: false }\n";
    let (mut session, marks) = session(&[("config.yaml", off)]);
    let (_place, panel) = place("omorfos-panel");
    let wallpaper = panel.with_file_name("omorfos-wallpaper");
    // One that fails as soon as it starts, as long as it is tried.
    let failing = "#!/bin/sh\nprintf f >> \"$(dirname \"$OMORFOS_SOCKET\")/panel\"\nexit 1\n";
    written(&panel, failing);
    install(&wallpaper, "wallpaper", "1");
    session.compositor_mut().set_panel(&panel);
    session.compositor_mut().set_wallpaper(&wallpaper);
    session.compositor_mut().autostart();
    session.eventually(|session| {
        let problems = session.compositor().problems();
        problems
            .iter()
            .any(|problem| problem.contains("keeps stopping"))
    });
    assert_eq!(marks("panel"), "ffff");
    // Its new version is started, though the one before was given up on.
    install(&panel, "panel", "2");
    install(&wallpaper, "wallpaper", "2");
    session.eventually(|_| marks("panel") == "ffff2");
    session.advance(Duration::from_secs(2));
    assert_eq!(marks("wallpaper"), "", "never started, as it is off");
}

#[test]
fn a_new_version_of_the_compositor_itself_is_said_to_need_logging_in_again() {
    let (mut session, _) = session(&[]);
    let (_place, own) = place("omorfos");
    written(&own, "one");
    session.compositor_mut().set_own(&own);
    session.compositor_mut().autostart();
    written(&own, "two");
    let said = |session: &Session| {
        let problems = session.compositor().problems();
        let said = problems
            .iter()
            .filter(|problem| problem.contains("log in again"));
        said.count()
    };
    session.eventually(|session| said(session) == 1);
    // Once, however often it is installed.
    written(&own, "three");
    session.advance(Duration::from_secs(2));
    assert_eq!(said(&session), 1);
}
