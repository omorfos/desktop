//! The pictures the shell's programs draw, put in memory they share with
//! the compositor by `omorfos_shell::pixels`.

use crate::common::{HEIGHT, Session, WIDTH};
use crate::windows::RED;

const GREEN: u32 = 0xff00_ff00;

#[test]
fn a_picture_of_the_wrong_length_is_refused_before_the_compositor_is_asked() {
    let mut session = Session::new();
    let mut client = session.connect();

    // Fifteen bytes are short of a two by two picture by one.
    let refused = client.shell_pixels((2, 2), &[0; 15]);

    assert_eq!(
        refused.unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    // Asked for a pool too small for the picture, the compositor would
    // have sent the program away.
    session.roundtrip(&mut client);
    assert_eq!(client.error(), None);
}

#[test]
fn a_picture_the_shell_makes_is_shown_as_it_was_drawn() {
    let mut session = Session::new();
    let mut client = session.connect();
    let window = session.open(&mut client, "app", (200, 100), RED);
    let shown = session.draw();
    let points = (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| (x, y)));
    let (x, y) = points
        .into_iter()
        .find(|&(x, y)| shown.at(x, y) == RED)
        .unwrap();

    let green = GREEN.to_le_bytes().repeat(200 * 100);
    let picture = client.shell_pixels((200, 100), &green).unwrap();
    window.surface.attach(Some(&picture), 0, 0);
    window.surface.damage_buffer(0, 0, 200, 100);
    window.surface.commit();
    session.roundtrip(&mut client);

    assert_eq!(session.draw().at(x, y), GREEN);
    assert_eq!(client.error(), None);
}
