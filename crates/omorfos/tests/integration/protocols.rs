//! What the compositor offers programs: the list in `omorfos-ipc` that the
//! relay is built from (ADR 60), and nothing else.

use omorfos::Roles;
use omorfos_ipc::protocols::{OFFERED, X_SERVER};

use crate::common::Session;
use crate::gpu::drawable;

#[test]
fn a_program_is_offered_what_is_listed_at_the_version_listed_and_nothing_else() {
    let mut session = Session::new();
    // What is offered only once the backend says a card can be drawn from.
    session
        .compositor_mut()
        .offer_gpu_buffers(0, drawable())
        .unwrap();
    // Every role, so that nothing is withheld from it.
    let client = session.connect_as(Roles::ALL);

    let versions = client.state.versions.iter().copied();
    let mut offered: Vec<(&str, u32)> = client
        .state
        .offered
        .iter()
        .map(String::as_str)
        .zip(versions)
        .collect();
    // A screen is offered once for each there is.
    offered.sort_unstable();
    offered.dedup();
    // The X server's own is checked where the X server is.
    let listed = OFFERED
        .iter()
        .filter(|protocol| protocol.interface != X_SERVER);
    let listed: Vec<(&str, u32)> = listed
        .map(|protocol| (protocol.interface, protocol.version))
        .collect();
    assert_eq!(offered, listed);
}
