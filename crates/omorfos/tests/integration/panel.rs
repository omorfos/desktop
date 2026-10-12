//! The panel, as a client of a real compositor.
//!
//! What a bar has on it and how it is drawn is tested with the panel
//! itself. Here it is the program: that its bars get onto the screens, keep
//! up with the session and the theme, and that clicks reach the compositor.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use omorfos::{Input, Scale};
use omorfos_config::{Color, Part, States, Theme};
use omorfos_ipc::{Client as Ipc, Reply, Request, State, Workspace, encode};
use omorfos_panel::{Bar, Panel};
use omorfos_ui::Text;
use serde_json::json;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{HEIGHT, Picture, Session, WIDTH};
use crate::windows::{BACKGROUND, BLUE, RED};

const LEFT: u32 = 0x110;
const RIGHT: u32 = 0x111;
/// What the panel is told the time is.
const TIME: &str = "14:05";
/// How tall the default theme makes a bar.
const BAR: i32 = 28;

fn argb(color: Color) -> u32 {
    u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
}

/// The colour of a bar where there is nothing on it.
fn plain() -> u32 {
    argb(Theme::default().style(Part::Panel, States::NONE).background)
}

/// The colour of the line under what is lit.
fn accent() -> u32 {
    argb(
        Theme::default()
            .style(Part::NoticeHeading, States::NONE)
            .color,
    )
}

/// A panel connected to the session over `wayland` and to these two ends
/// of its own socket.
fn panel_on(session: &mut Session, news: Ipc, commands: Ipc, config: &Path) -> Panel {
    let (ours, theirs) = UnixStream::pair().unwrap();
    session.compositor_mut().insert_client(ours).unwrap();
    Panel::new(theirs, news, commands, config).unwrap()
}

/// A panel in the session, as it runs in use.
fn panel(session: &mut Session) -> Panel {
    let socket = session.listen();
    let news = Ipc::connect(&socket).unwrap();
    let commands = Ipc::connect(&socket).unwrap();
    let config = session.config.path().to_owned();
    panel_on(session, news, commands, &config)
}

/// Lets the panel and the compositor talk until they have nothing more to
/// say to each other.
fn settle(session: &mut Session, panel: &mut Panel) {
    for _ in 0..20 {
        panel.turn(TIME).unwrap();
        session.step();
    }
}

fn move_to(session: &mut Session, x: i32, y: i32) {
    let (x, y) = (f64::from(x), f64::from(y));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
}

fn click(session: &mut Session, button: u32) {
    for pressed in [true, false] {
        let input = Input::Button { button, pressed };
        session.compositor_mut().input(input, 0);
    }
}

/// What the compositor says of the session.
fn state(session: &mut Session) -> State {
    let mut peer = Ipc::connect(&session.config.path().join("ipc.sock")).unwrap();
    let Reply::State(state) = session.ask(&mut peer, &Request::State) else {
        unreachable!("the compositor answers a question about the state with it");
    };
    state
}

/// Where along the first screen's bar a click asks for `request`.
fn button(session: &mut Session, request: &Request) -> i32 {
    let state = state(session);
    let width = u32::try_from(WIDTH).unwrap();
    let bar = Bar::new(
        &state,
        "test-0",
        width,
        TIME,
        &Theme::default(),
        &mut Text::new(),
    );
    let found = (0..WIDTH).find(|x| bar.click(*x).as_ref() == Some(request));
    // A little way in from its edge.
    found.unwrap() + 3
}

fn workspace(number: u32) -> Request {
    Request::Perform {
        action: json!({ "workspace": number }),
    }
}

/// The numbers of the workspaces the first screen is showing.
fn shown(session: &mut Session) -> Vec<u32> {
    let workspaces = state(session).workspaces.into_iter();
    let active = workspaces.filter(|workspace| workspace.active);
    active.map(|workspace| workspace.number).collect()
}

/// How many rows up from the bottom of a picture are a bar's, going by its
/// right-hand end, where there is never a button.
fn bar_height(picture: &Picture, (width, height): (i32, i32)) -> i32 {
    let rows = (0..height).rev();
    let bar = rows.take_while(|y| picture.at(width - 1, *y) == plain());
    i32::try_from(bar.count()).unwrap()
}

#[test]
fn the_panel_puts_a_bar_along_the_bottom_of_the_screen() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    assert_eq!(bar_height(&session.draw(), (WIDTH, HEIGHT)), 0);

    settle(&mut session, &mut panel);

    let picture = session.draw();
    assert_eq!(bar_height(&picture, (WIDTH, HEIGHT)), BAR);
    assert_eq!(picture.at(WIDTH - 1, HEIGHT - BAR - 1), BACKGROUND);
    // The first workspace is the one shown, and is underlined.
    assert_eq!(picture.at(3, HEIGHT - 1), accent());
    let second = button(&mut session, &workspace(2));
    assert_eq!(picture.at(second, HEIGHT - 1), plain());
}

#[test]
fn windows_are_kept_clear_of_the_bar() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);

    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Maximize);
    session.roundtrip(&mut client);

    let ((_, height), _) = client.last_configure();
    assert!(height <= HEIGHT - BAR, "{height}");
    assert!(height > HEIGHT - BAR * 3, "{height}");
}

#[test]
fn clicking_a_workspace_on_the_bar_shows_it() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);
    assert_eq!(shown(&mut session), [1]);
    let second = button(&mut session, &workspace(2));

    move_to(&mut session, 3, HEIGHT - 5);
    move_to(&mut session, second, HEIGHT - 5);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);

    assert_eq!(shown(&mut session), [2]);
    // And the bar has it that way.
    let picture = session.draw();
    assert_eq!(picture.at(second, HEIGHT - 1), accent());
    assert_eq!(picture.at(3, HEIGHT - 1), plain());
}

#[test]
fn only_a_click_with_the_left_button_on_a_button_asks_for_anything() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);
    let second = button(&mut session, &workspace(2));

    // The other button.
    move_to(&mut session, second, HEIGHT - 5);
    click(&mut session, RIGHT);
    settle(&mut session, &mut panel);
    assert_eq!(shown(&mut session), [1]);

    // Where there is no button.
    move_to(&mut session, WIDTH - 3, HEIGHT - 5);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);
    assert_eq!(shown(&mut session), [1]);

    // Having been over a button and left the bar.
    move_to(&mut session, second, HEIGHT - 5);
    move_to(&mut session, second, 50);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);
    assert_eq!(shown(&mut session), [1]);
}

#[test]
fn clicking_a_window_on_the_bar_brings_it_up() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);
    let mut client = session.connect();
    session.open(&mut client, "first", (200, 100), RED);
    session.open(&mut client, "second", (200, 100), BLUE);
    settle(&mut session, &mut panel);
    let windows = state(&mut session).windows;
    assert!(!windows[0].focused && windows[1].focused);
    let first = button(
        &mut session,
        &Request::Activate {
            window: windows[0].id,
        },
    );
    let second = button(
        &mut session,
        &Request::Activate {
            window: windows[1].id,
        },
    );
    // The one with focus is underlined.
    let picture = session.draw();
    assert_eq!(picture.at(first, HEIGHT - 1), plain());
    assert_eq!(picture.at(second, HEIGHT - 1), accent());

    move_to(&mut session, first, HEIGHT - 5);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);

    let windows = state(&mut session).windows;
    assert!(windows[0].focused && !windows[1].focused);
    let picture = session.draw();
    assert_eq!(picture.at(first, HEIGHT - 1), accent());
    assert_eq!(picture.at(second, HEIGHT - 1), plain());
}

#[test]
fn every_screen_has_a_bar_of_its_own() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);

    let other = session.add_output((800, 600));
    settle(&mut session, &mut panel);
    assert_eq!(bar_height(&session.draw_on(1), (800, 600)), BAR);
    assert_eq!(bar_height(&session.draw(), (WIDTH, HEIGHT)), BAR);

    // A click on the second screen's bar is about the second screen.
    let second = button(&mut session, &workspace(2));
    move_to(&mut session, WIDTH + second, 600 - 5);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);
    assert_eq!(shown(&mut session), [1, 2]);
    assert_eq!(session.draw_on(1).at(second, 600 - 1), accent());
    assert_eq!(session.draw().at(second, HEIGHT - 1), plain());

    // The screen goes, and the panel carries on with the one that is left.
    session.compositor_mut().remove_output(&other);
    settle(&mut session, &mut panel);
    move_to(&mut session, second, HEIGHT - 5);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);
    assert_eq!(shown(&mut session), [2]);
    assert_eq!(session.draw().at(second, HEIGHT - 1), accent());
}

#[test]
fn a_bar_is_drawn_with_as_many_pixels_as_its_screen_has() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);

    session.suggest_scale(Scale::new(2.0));
    settle(&mut session, &mut panel);

    let picture = session.draw();
    assert_eq!(bar_height(&picture, (WIDTH, HEIGHT)), BAR * 2);
    // The line under the workspace is twice as thick too, and sharp.
    assert_eq!(picture.at(6, HEIGHT - 4), accent());
    assert_ne!(picture.at(6, HEIGHT - 5), accent());
}

#[test]
fn the_bar_follows_the_theme_as_it_is_saved() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);
    let panel = RefCell::new(panel);

    session.write_config(
        "theme.css",
        "panel { height: 40px; background: #102030; }\n\
         workspace, task { background: #102030; }\n",
    );

    session.eventually(|session| {
        panel.borrow_mut().turn(TIME).unwrap();
        session.step();
        let picture = session.draw();
        let rows = (HEIGHT - 40..HEIGHT).rev();
        let themed = |y: i32| picture.at(WIDTH - 1, y) == 0xff10_2030;
        rows.clone().all(themed) && !themed(HEIGHT - 41)
    });
}

#[test]
fn a_bar_with_a_margin_floats_clear_of_the_edge_and_windows_keep_clear_of_both() {
    let mut session = Session::with_config(&[(
        "theme.css",
        "panel { margin: 0 20px 6px 20px; border-radius: 8px; }\n",
    )]);
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);

    let picture = session.draw();
    // The desktop shows below the bar and at either end of it.
    assert_eq!(picture.at(WIDTH / 2, HEIGHT - 1), BACKGROUND);
    assert_eq!(picture.at(WIDTH / 2, HEIGHT - 6), BACKGROUND);
    assert_eq!(picture.at(WIDTH / 2, HEIGHT - 7), plain());
    assert_eq!(picture.at(WIDTH / 2, HEIGHT - 6 - BAR), plain());
    assert_eq!(picture.at(WIDTH / 2, HEIGHT - 7 - BAR), BACKGROUND);
    assert_eq!(picture.at(19, HEIGHT - 20), BACKGROUND);
    assert_eq!(picture.at(WIDTH - 20, HEIGHT - 20), BACKGROUND);
    // Its corners are rounded, where no button is in them.
    assert_eq!(picture.at(WIDTH - 21, HEIGHT - 7), BACKGROUND);
    assert_eq!(picture.at(WIDTH - 21, HEIGHT - 6 - BAR), BACKGROUND);

    // A maximized window stops above the bar and its margin.
    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), RED);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Maximize);
    session.roundtrip(&mut client);
    let ((_, height), _) = client.last_configure();
    assert!(height <= HEIGHT - BAR - 6, "{height}");
}

/// Whether anything is written at the right-hand end of the first screen's
/// bar, where the clock is.
fn clock_shows(picture: &Picture) -> bool {
    let columns = WIDTH - 100..WIDTH;
    let mut places = columns.flat_map(|x| (HEIGHT - BAR..HEIGHT).map(move |y| (x, y)));
    places.any(|(x, y)| picture.at(x, y) != plain())
}

#[test]
fn every_bar_has_a_clock_that_is_drawn_again_only_when_the_time_changes() {
    let mut session = Session::new();
    let mut panel = panel(&mut session);
    settle(&mut session, &mut panel);
    let before = session.draw();
    assert!(clock_shows(&before));

    // Time passes within the minute.
    settle(&mut session, &mut panel);
    assert!(!session.draw().damaged);

    // The minute turns.
    for _ in 0..20 {
        panel.turn("14:06").unwrap();
        session.step();
    }
    let after = session.draw();
    assert!(after.damaged && clock_shows(&after));
    let columns = WIDTH - 100..WIDTH;
    let mut places = columns.flat_map(|x| (HEIGHT - BAR..HEIGHT).map(move |y| (x, y)));
    assert!(places.any(|(x, y)| before.at(x, y) != after.at(x, y)));
}

#[test]
fn a_panel_without_a_configuration_directory_uses_the_default_theme() {
    let mut session = Session::new();
    let socket = session.listen();
    let news = Ipc::connect(&socket).unwrap();
    let commands = Ipc::connect(&socket).unwrap();
    let nowhere = session.config.path().join("absent");
    let mut panel = panel_on(&mut session, news, commands, &nowhere);

    settle(&mut session, &mut panel);

    assert_eq!(bar_height(&session.draw(), (WIDTH, HEIGHT)), BAR);
}

/// A panel whose connections to the compositor's own socket end here
/// rather than at the compositor.
fn panel_told_by_us(session: &mut Session) -> (Panel, UnixStream, UnixStream) {
    let (news, our_news) = UnixStream::pair().unwrap();
    let (commands, our_commands) = UnixStream::pair().unwrap();
    let config = session.config.path().to_owned();
    let news = Ipc::from_stream(news);
    let commands = Ipc::from_stream(commands);
    let panel = panel_on(session, news, commands, &config);
    (panel, our_news, our_commands)
}

fn said(state: &State) -> String {
    encode(&Reply::State(state.clone()))
}

#[test]
fn the_panel_asks_for_news_and_shows_what_it_is_told() {
    let mut session = Session::new();
    let (mut panel, mut news, commands) = panel_told_by_us(&mut session);
    let mut asked = String::new();
    BufReader::new(&news).read_line(&mut asked).unwrap();
    assert_eq!(asked, encode(&Request::Subscribe));

    // Before it has heard anything, the bar is there with nothing on it.
    settle(&mut session, &mut panel);
    let picture = session.draw();
    assert_eq!(bar_height(&picture, (WIDTH, HEIGHT)), BAR);
    assert_eq!(picture.at(3, HEIGHT - 1), plain());

    // Anything that is not news is passed over.
    let state = State {
        workspaces: vec![Workspace {
            output: "test-0".to_owned(),
            number: 7,
            name: "seven".to_owned(),
            active: true,
        }],
        ..State::default()
    };
    news.write_all(encode(&Reply::Done).as_bytes()).unwrap();
    news.write_all(said(&state).as_bytes()).unwrap();
    settle(&mut session, &mut panel);
    assert_eq!(session.draw().at(3, HEIGHT - 1), accent());

    // A click asks for what the button stands for, in the protocol's words.
    move_to(&mut session, 3, HEIGHT - 5);
    click(&mut session, LEFT);
    settle(&mut session, &mut panel);
    let mut asked = String::new();
    BufReader::new(&commands).read_line(&mut asked).unwrap();
    assert_eq!(asked, encode(&workspace(7)));

    // News that changes nothing on this screen's bar draws nothing.
    session.draw();
    let mut elsewhere = state.clone();
    elsewhere.workspaces.push(Workspace {
        output: "another".to_owned(),
        ..state.workspaces[0].clone()
    });
    news.write_all(said(&elsewhere).as_bytes()).unwrap();
    settle(&mut session, &mut panel);
    assert!(!session.draw().damaged);
}

#[test]
fn the_panel_gives_up_when_the_compositor_stops_telling_it_anything() {
    let mut session = Session::new();
    let (mut panel, news, _commands) = panel_told_by_us(&mut session);
    settle(&mut session, &mut panel);

    drop(news);

    assert!(panel.turn(TIME).is_err());
}

#[test]
fn the_panel_gives_up_when_it_cannot_ask_for_what_was_clicked() {
    let mut session = Session::new();
    let (mut panel, mut news, commands) = panel_told_by_us(&mut session);
    let state = State {
        workspaces: vec![Workspace {
            output: "test-0".to_owned(),
            number: 1,
            name: "one".to_owned(),
            active: true,
        }],
        ..State::default()
    };
    news.write_all(said(&state).as_bytes()).unwrap();
    settle(&mut session, &mut panel);

    drop(commands);
    move_to(&mut session, 3, HEIGHT - 5);
    click(&mut session, LEFT);
    session.step();

    let turns = (0..20).map(|_| panel.turn(TIME));
    assert!(turns.into_iter().any(|turn| turn.is_err()));
}

#[test]
fn the_panel_gives_up_when_the_compositor_has_gone() {
    let mut session = Session::new();
    let (mut panel, _news, _commands) = panel_told_by_us(&mut session);
    settle(&mut session, &mut panel);

    drop(session);

    // The connection closing can take a moment to be seen.
    let gone = (0..5000).any(|_| {
        std::thread::sleep(Duration::from_millis(1));
        panel.turn(TIME).is_err()
    });
    assert!(gone);
}

#[test]
fn the_panel_runs_by_itself_in_the_session_it_is_started_in() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session.inner().serve(socket).unwrap();
    let ipc = session.listen();
    // The configuration is looked for where the session would have it.
    std::fs::create_dir(runtime.join("omorfos")).unwrap();
    let theme = runtime.join("omorfos/theme.css");

    let panel = std::thread::spawn(move || {
        let runtime = Some(runtime.as_os_str());
        let display = Some(std::ffi::OsStr::new("wayland-test"));
        omorfos_panel::start(runtime, display, Some(ipc.as_os_str()), runtime, None)
    });

    session.eventually(|session| bar_height(&session.draw(), (WIDTH, HEIGHT)) == BAR);
    // It keeps up without being prodded: asleep until there is news.
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Workspace(
            omorfos_config::WorkspaceNumber::try_from(2).unwrap(),
        ));
    session.eventually(|session| {
        let picture = session.draw();
        picture.at(3, HEIGHT - 1) == plain() && bar_height(&picture, (WIDTH, HEIGHT)) == BAR
    });

    // And follows the theme.
    std::fs::write(theme, "panel { height: 36px; }\n").unwrap();
    session.eventually(|session| bar_height(&session.draw(), (WIDTH, HEIGHT)) == 36);

    // It ends with the session, saying why.
    drop(session);
    let failure = panel.join().unwrap();
    assert_ne!(failure.to_string(), "");
}
