//! Glass: what is behind a part, blurred, under it where its background
//! lets it be seen (`backdrop-filter`, ADR 33).

use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

use omorfos::{Frosting, Input, Roles, Software};
use omorfos_config::Action;
use omorfos_ui::Blur;
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::utils::{Physical, Rectangle, Size};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::xdg::shell::client::xdg_popup::XdgPopup;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, Drawn, HEIGHT, LayerShape, Session, Toplevel, WIDTH, key};
use crate::windows::{BACKGROUND, BLUE, RED};

/// A theme in which a panel is glass: white at half, over what is behind
/// it blurred by 8 pixels.
pub(crate) const GLASS: &str = "panel { background: rgba(255, 255, 255, 0.5); \
                     backdrop-filter: blur(8px); box-shadow: none; }\n";

/// White at half, premultiplied, as the panel draws its tint.
pub(crate) const TINT: u32 = 0x8080_8080;

/// A panel across the top of the screen, `height` high, over the windows
/// and keeping no room from them.
pub(crate) fn across(height: u32) -> LayerShape {
    LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, height),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    }
}

/// How red a pixel is.
fn red(pixel: u32) -> u32 {
    (pixel >> 16) & 0xff
}

/// A session with `theme`, a red window 200 by 100 in the top left corner,
/// and a panel of glass 30 high across the top, over it.
fn over_a_red_window(theme: &str) -> (Session, (Client, Toplevel), Client) {
    let mut session = Session::with_config(&[("theme.css", theme)]);
    session.blurring();
    let mut window = session.connect();
    let toplevel = session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    (session, (window, toplevel), panel)
}

#[test]
fn glass_of_two_blurs_over_the_desktop_alone_is_not_blurred_again_while_nothing_changes() {
    // A window that asks for what is behind it to be blurred, which the
    // theme here does by 24, and a panel that blurs by 8, each with
    // nothing but the desktop behind it, and too far apart for either to
    // reach the other.
    let theme = format!(
        "{GLASS}window, window:focus {{ box-shadow: none; backdrop-filter: blur(24px); }}\n"
    );
    let mut session = Session::with_config(&[("theme.css", &theme)]);
    session.blurring();
    let mut window = session.connect();
    let toplevel = session.open(&mut window, "app", (200, 100), 0);
    let _effect = window.blur_behind(&toplevel.surface, &[(0, 0, 200, 100)]);
    toplevel.surface.commit();
    session.roundtrip(&mut window);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    let low = LayerShape {
        anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
        ..across(30)
    };
    session.open_layer(&mut panel, &low, (WIDTH, 30), TINT);
    session.settle();
    drop(session.draw());
    let blurs = session.compositor().blurs();
    assert_eq!(blurs, 2, "the desktop, blurred once for each");
    for _ in 0..3 {
        session.step();
        let drawn = session.draw();
        assert!(!drawn.damaged, "nothing to draw again");
    }
    assert_eq!(session.compositor().blurs(), blurs, "nor blurred again");
}

#[test]
fn a_panel_of_glass_is_redder_over_a_red_window_and_only_there() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    let picture = session.draw();
    let (over, beside) = (picture.at(100, 15), picture.at(400, 15));
    assert!(red(over) > red(beside) + 64, "{over:#x} {beside:#x}");
    // To the edge of the screen, beyond which is what is inside it,
    // mirrored.
    assert_eq!(picture.at(100, 0), over, "as red at the top");
    // What is behind it is blurred: its edge is soft, where it is sharp
    // under it.
    assert!(red(picture.at(196, 15)) < red(over), "softened inside");
    assert!(red(picture.at(204, 15)) > red(beside), "and out");
    assert_eq!(picture.at(100, 60), RED, "the window, below it");
    assert_eq!(picture.at(400, 60), BACKGROUND, "the desktop, below it");
}

#[test]
fn glass_asked_to_blur_too_little_to_hide_what_is_behind_it_blurs_as_little_as_glass_may() {
    let slight = GLASS.replace("blur(8px)", "blur(2px)");
    let least = GLASS.replace("blur(8px)", "blur(16px)");
    let more = GLASS.replace("blur(8px)", "blur(24px)");
    let across_the_edge = |theme: &str| {
        let (mut session, _window, _panel) = over_a_red_window(theme);
        let picture = session.draw();
        (150..250).map(|x| picture.at(x, 15)).collect::<Vec<_>>()
    };
    assert_eq!(across_the_edge(&slight), across_the_edge(&least));
    assert_ne!(across_the_edge(&more), across_the_edge(&least), "and more");
}

#[test]
fn glass_is_not_drawn_again_or_blurred_again_while_nothing_behind_it_changes() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    drop(session.draw());
    let blurs = session.compositor().blurs();
    assert!(blurs > 0);
    session.step();
    assert!(!session.draw().damaged, "not drawn again");
    assert_eq!(session.compositor().blurs(), blurs, "nor blurred again");
}

#[test]
fn glass_is_blurred_again_when_what_is_behind_it_changes() {
    let (mut session, (mut window, toplevel), _panel) = over_a_red_window(GLASS);
    let reddish = session.draw().at(100, 15);
    let blurs = session.compositor().blurs();
    window.fill(&toplevel.surface, (200, 100), BLUE);
    session.roundtrip(&mut window);
    session.roundtrip(&mut window);
    let bluish = session.draw().at(100, 15);
    assert!(session.compositor().blurs() > blurs, "blurred again");
    assert!(red(bluish) < red(reddish), "{bluish:#x} {reddish:#x}");
    assert!(bluish & 0xff > reddish & 0xff, "{bluish:#x} {reddish:#x}");
}

#[test]
fn glass_over_the_desktop_alone_takes_what_is_behind_it_from_one_blur_of_it() {
    // Along the bottom, and in the middle of the right side: clear of each
    // other and of where windows open, in the top left corner.
    let mut session = Session::with_config(&[("theme.css", GLASS)]);
    session.blurring();
    let mut bottom = session.connect();
    bottom.layer_name = "panel";
    let low = LayerShape {
        anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
        ..across(40)
    };
    session.open_layer(&mut bottom, &low, (WIDTH, 40), TINT);
    let mut side = session.connect();
    side.layer_name = "panel";
    let right = LayerShape {
        anchor: Anchor::Right,
        size: (40, 100),
        ..across(40)
    };
    session.open_layer(&mut side, &right, (40, 100), TINT);
    let picture = session.draw();
    assert_eq!(picture.at(300, 470), picture.at(620, 240), "the same glass");
    assert_ne!(picture.at(300, 470), BACKGROUND, "tinted");
    assert_eq!(session.compositor().blurs(), 1, "one blur, of the desktop");
    // A window opening clear of them blurs nothing more.
    let mut window = session.connect();
    session.open(&mut window, "app", (100, 100), RED);
    session.step();
    drop(session.draw());
    assert_eq!(session.compositor().blurs(), 1);
}

#[test]
fn glass_where_blur_cannot_be_drawn_is_solid_in_its_colour() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    session.frosting = None;
    session.step();
    let picture = session.draw();
    // Its tint over white, which is white, over the window and beside it.
    assert_eq!(picture.at(100, 15), 0xffff_ffff);
    assert_eq!(picture.at(400, 15), 0xffff_ffff);
    assert_eq!(session.compositor().blurs(), 0, "and nothing is blurred");
}

#[test]
fn glass_has_its_corners_rounded_as_its_part_does() {
    let rounded = "panel { background: rgba(255, 255, 255, 0.5); border-radius: 12px; \
                   backdrop-filter: blur(8px); box-shadow: none; }\n";
    let (mut session, _window, _panel) = over_a_red_window(rounded);
    session.frosting = None;
    session.step();
    let picture = session.draw();
    // In the corner, nothing of the glass: the tint over the window as it
    // is. Within, the tint over white.
    assert_eq!(picture.at(0, 0), 0xffff_8080, "the tint over red");
    assert_eq!(picture.at(20, 15), 0xffff_ffff);
    assert_eq!(
        picture.at(WIDTH - 1, 0),
        0xff86_8993,
        "the tint over the desktop"
    );
}

#[test]
fn a_part_whose_background_cannot_be_seen_through_has_no_glass() {
    let opaque = "panel { background: #ffffff; backdrop-filter: blur(8px); }\n";
    let (mut session, _window, _panel) = over_a_red_window(opaque);
    drop(session.draw());
    assert_eq!(session.compositor().blurs(), 0);
}

#[test]
fn glass_is_solid_where_transparency_is_to_be_reduced() {
    let mut session = Session::with_config(&[
        ("theme.css", GLASS),
        ("config.yaml", "appearance: { reduce_transparency: true }"),
    ]);
    session.blurring();
    let mut window = session.connect();
    session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    let picture = session.draw();
    assert_eq!(picture.at(100, 15), 0xffff_ffff);
    assert_eq!(session.compositor().blurs(), 0);
}

#[test]
fn a_menu_of_glass_over_a_red_window_is_red_there_and_not_beside_it() {
    let theme = "window, window:focus { box-shadow: none; }\n\
                 menu { background: rgba(255, 255, 255, 0.5); \
                 backdrop-filter: blur(8px); box-shadow: none; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    session.blurring();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 400), RED);
    // Its corner at (100, 100), 220 wide: half over the window.
    let motion = Input::Motion { x: 100.0, y: 100.0 };
    session.compositor_mut().input(motion, 0);
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    let picture = session.draw();
    // Red more than green, whatever is written on it there.
    let cast = |pixel: u32| i64::from(red(pixel)) - i64::from((pixel >> 8) & 0xff);
    assert!(
        cast(picture.at(150, 104)) > 64,
        "{:#x}",
        picture.at(150, 104)
    );
    assert!(
        cast(picture.at(300, 104)).abs() < 16,
        "{:#x}",
        picture.at(300, 104)
    );
    assert!(session.compositor().blurs() > 0);
}

/// A blur as the software's, that counts the times it is given nothing it
/// made before to make it in again.
struct Counting(Rc<std::cell::Cell<u32>>);

impl Frosting<PixmanRenderer, Drawn> for Counting {
    fn blur(
        &mut self,
        renderer: &mut PixmanRenderer,
        picture: (&mut Drawn, Size<i32, Physical>),
        blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool {
        self.0.set(self.0.get() + u32::from(made.is_none()));
        Frosting::<PixmanRenderer, Drawn>::blur(&mut Software, renderer, picture, blur, made)
    }

    fn element(
        &self,
        renderer: &mut PixmanRenderer,
        made: &dyn Any,
        at: (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<PixmanRenderer>>> {
        Frosting::<PixmanRenderer, Drawn>::element(&Software, renderer, made, at)
    }
}

#[test]
fn a_menu_of_glass_coming_in_over_a_window_keeps_its_picture_as_it_grows() {
    // As it comes in, from a little smaller than it is, over 100 ms.
    let theme = "window, window:focus { box-shadow: none; }\n\
                 menu { background: rgba(255, 255, 255, 0.5); transition: 100ms linear; \
                 backdrop-filter: blur(8px); box-shadow: none; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    let fresh = Rc::default();
    session.frosting = Some(Box::new(Counting(Rc::clone(&fresh))));
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 400), RED);
    session.advance(Duration::from_millis(200));
    drop(session.draw());
    let motion = Input::Motion { x: 100.0, y: 100.0 };
    session.compositor_mut().input(motion, 0);
    session.compositor_mut().perform(&Action::ShowMenu);
    let before = fresh.get();
    for _ in 0..10 {
        session.advance(Duration::from_millis(10));
        drop(session.draw());
    }
    // Blurred on each frame, as it grows over what is behind it; what it is
    // blurred in is made once.
    assert_eq!(fresh.get() - before, 1);
}

#[test]
fn a_menu_fading_out_keeps_its_glass_as_it_goes() {
    let theme = "window, window:focus { box-shadow: none; }\n\
                 menu { background: rgba(255, 255, 255, 0.5); transition: 100ms linear; \
                 backdrop-filter: blur(8px); box-shadow: none; }\n";
    let mut session = Session::with_config(&[("theme.css", theme)]);
    session.blurring();
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 400), RED);
    // Its corner at (100, 100), 220 wide: half over the window.
    let motion = Input::Motion { x: 100.0, y: 100.0 };
    session.compositor_mut().input(motion, 0);
    session.compositor_mut().perform(&Action::ShowMenu);
    session.step();
    session.advance(Duration::from_millis(100));
    drop(session.draw());
    for pressed in [true, false] {
        let code = key::ESCAPE;
        session
            .compositor_mut()
            .input(Input::Key { code, pressed }, 0);
    }
    session.step();
    session.advance(Duration::from_millis(50));
    // Half gone, the window's red still runs under it past the window's
    // edge, blurred, which a tint alone would not show.
    let picture = session.draw();
    let (past, beside) = (picture.at(204, 104), picture.at(300, 104));
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
}

#[test]
fn a_panel_fading_out_keeps_its_glass_as_it_goes() {
    let theme = format!("{GLASS}panel {{ transition: 100ms linear; }}\n");
    let mut session = Session::with_config(&[("theme.css", theme.as_str())]);
    session.blurring();
    let mut window = session.connect();
    session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    let (surface, role) = session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    session.advance(Duration::from_millis(100));
    drop(session.draw());
    role.destroy();
    surface.destroy();
    session.roundtrip(&mut panel);
    session.step();
    session.advance(Duration::from_millis(50));
    let picture = session.draw();
    let (past, beside) = (picture.at(204, 15), picture.at(400, 15));
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
}

/// A theme in which a titlebar is glass, white at half over what is behind
/// it blurred by 8 pixels, and a window casts no shadow and takes a moment
/// to go.
const TITLEBAR: &str = "window, window:focus { box-shadow: none; transition: 100ms linear; }\n\
                        titlebar, titlebar:focus { background: rgba(255, 255, 255, 0.5); \
                        backdrop-filter: blur(8px); }\n";

/// The row of the screen a titlebar's glass is tried along: near its top,
/// above its title, in the titlebar of a window cascaded from the corner.
const ALONG: i32 = 35;

/// A session with `theme`, a red window 200 by 400 in the top left corner,
/// and over it a blue one 400 by 100 with a titlebar, cascaded from the
/// corner, whose titlebar runs from 32 down to 80.
fn a_titlebar_over_a_red_window(theme: &str) -> (Session, [(Client, Toplevel); 2]) {
    let mut session = Session::with_config(&[("theme.css", theme)]);
    session.blurring();
    let mut red = session.connect();
    let under = session.open(&mut red, "red", (200, 400), RED);
    let mut blue = session.connect();
    let over = session.open_decorated(&mut blue, "blue", (400, 100), BLUE);
    // Come, as windows take a moment to.
    session.advance(Duration::from_millis(100));
    (session, [(red, under), (blue, over)])
}

#[test]
fn a_titlebar_of_glass_is_redder_over_a_red_window_and_only_there() {
    let (mut session, _windows) = a_titlebar_over_a_red_window(TITLEBAR);
    let picture = session.draw();
    let (over, past, beside) = (
        picture.at(150, ALONG),
        picture.at(204, ALONG),
        picture.at(380, ALONG),
    );
    assert!(red(over) > red(beside) + 64, "{over:#x} {beside:#x}");
    // Blurred, the red runs on past the window's edge.
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
    assert!(session.compositor().blurs() > 0);
}

#[test]
fn a_titlebars_glass_blurs_as_far_as_it_is_asked_and_no_further() {
    // Blurred by as little as glass may, 16 pixels here: the red of the
    // window under it runs half a blur past its edge, at 200, and is all
    // but gone two and a half blurs past it.
    let (mut session, _windows) = a_titlebar_over_a_red_window(TITLEBAR);
    let picture = session.draw();
    let beside = red(picture.at(380, ALONG));
    let (near, far) = (red(picture.at(208, ALONG)), red(picture.at(240, ALONG)));
    assert!(near > beside + 16, "{near} {beside}");
    assert!(far <= beside + 4, "{far} {beside}");
}

#[test]
fn a_titlebars_glass_is_rounded_above_as_its_window_is_and_square_below() {
    let theme = format!("{TITLEBAR}window {{ border: none; }}\n");
    let (mut session, _windows) = a_titlebar_over_a_red_window(&theme);
    session.frosting = None;
    session.step();
    let picture = session.draw();
    // In the top corner, nothing of the titlebar or its glass: the red
    // window. In the bottom one, the tint over the glass, solid white.
    assert_eq!(picture.at(32, 32), RED, "rounded above");
    assert_eq!(picture.at(32, 79), 0xffff_ffff, "square below");
    assert_eq!(picture.at(32, 80), BLUE, "the window under it");
    // What is drawn on the titlebar is over its glass: the button that
    // closes the window, in its colour, at its left-hand end.
    assert_eq!(picture.at(52, 56), 0xfff8_616d, "the close button over it");
}

#[test]
fn a_window_rolled_up_into_its_titlebar_keeps_its_glass() {
    let (mut session, _windows) = a_titlebar_over_a_red_window(TITLEBAR);
    session.compositor_mut().perform(&Action::Shade);
    session.step();
    let picture = session.draw();
    let (over, beside) = (picture.at(150, ALONG), picture.at(380, ALONG));
    assert!(red(over) > red(beside) + 64, "{over:#x} {beside:#x}");
    assert_eq!(picture.at(300, 80), BACKGROUND, "nothing of it below");
}

#[test]
fn a_window_fading_out_keeps_its_titlebars_glass_as_it_goes() {
    let (mut session, _windows) = a_titlebar_over_a_red_window(TITLEBAR);
    session.advance(Duration::from_millis(100));
    drop(session.draw());
    session.compositor_mut().perform(&Action::Minimize);
    session.step();
    session.advance(Duration::from_millis(50));
    // Half gone, the red still runs past the red window's edge under it,
    // blurred, which a tint alone would not show.
    let picture = session.draw();
    let (past, beside) = (picture.at(204, ALONG), picture.at(380, ALONG));
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
}

#[test]
fn the_tooltip_of_the_mark_is_glass_if_the_theme_makes_it_so() {
    for (glass, blurred) in [("blur(8px)", true), ("none", false)] {
        let theme = format!(
            "* {{ transition: none; }}\n\
             tooltip {{ background: rgba(255, 255, 255, 0.5); \
             backdrop-filter: {glass}; box-shadow: none; }}\n"
        );
        let mut session = Session::with_config(&[("theme.css", theme.as_str())]);
        session.blurring();
        let mut camera = session.connect_as(Roles::CAPTURE);
        let _screen = camera.capture(0, false);
        session.roundtrip(&mut camera);
        drop(session.draw());
        assert_eq!(session.compositor().blurs(), 0, "nothing glass yet");
        // The pointer resting on the mark, at the top right.
        let motion = Input::Motion {
            x: f64::from(WIDTH - 20),
            y: 16.0,
        };
        session.compositor_mut().input(motion, 0);
        session.step();
        session.step();
        assert_ne!(session.compositor().mark_tooltip(), [""; 0], "it is up");
        drop(session.draw());
        assert_eq!(session.compositor().blurs() > 0, blurred, "{glass}");
    }
}

#[test]
fn the_prompt_that_asks_for_a_secret_is_glass_if_the_theme_makes_its_question_so() {
    for (glass, blurred) in [("blur(8px)", true), ("none", false)] {
        // The session under the dim is not blurred, so that what is
        // blurred is the question's alone.
        let theme = format!(
            "question {{ background: rgba(255, 255, 255, 0.5); \
             backdrop-filter: {glass}; box-shadow: none; }}\n\
             sealed {{ backdrop-filter: none; }}\n"
        );
        let mut session = Session::with_config(&[("theme.css", theme.as_str())]);
        session.blurring();
        let mut app = session.connect_plain();
        session.open(&mut app, "app", (200, 100), RED);
        drop(session.draw());
        assert_eq!(session.compositor().blurs(), 0, "nothing glass yet");
        // A prompt that draws nothing of its own over its glass.
        let mut asker = session.connect_as(Roles::ASK);
        let shape = LayerShape {
            layer: Layer::Overlay,
            anchor: Anchor::empty(),
            size: (100, 50),
            exclusive_zone: 0,
            keyboard: KeyboardInteractivity::Exclusive,
        };
        let _prompt = session.open_layer(&mut asker, &shape, (100, 50), 0);
        session.step();
        drop(session.draw());
        assert_eq!(session.compositor().blurs() > 0, blurred, "{glass}");
    }
}

/// A blur that says it blurred and has nothing to draw, as a shader might
/// whose textures could not be drawn with.
struct Blank;

impl Frosting<PixmanRenderer, Drawn> for Blank {
    fn blur(
        &mut self,
        _renderer: &mut PixmanRenderer,
        _picture: (&mut Drawn, Size<i32, Physical>),
        _blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool {
        *made = Some(Box::new(()));
        true
    }

    fn element(
        &self,
        _renderer: &mut PixmanRenderer,
        _made: &dyn Any,
        _at: (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<PixmanRenderer>>> {
        None
    }
}

/// A blur wider than it is asked for, as a shader that sampled further
/// apart than it should would make.
pub(crate) struct Wider;

impl Frosting<PixmanRenderer, Drawn> for Wider {
    fn blur(
        &mut self,
        renderer: &mut PixmanRenderer,
        picture: (&mut Drawn, Size<i32, Physical>),
        _blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool {
        Blur::new(16).is_some_and(|wider| {
            Frosting::<PixmanRenderer, Drawn>::blur(&mut Software, renderer, picture, &wider, made)
        })
    }

    fn element(
        &self,
        renderer: &mut PixmanRenderer,
        made: &dyn Any,
        at: (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<PixmanRenderer>>> {
        Frosting::<PixmanRenderer, Drawn>::element(&Software, renderer, made, at)
    }
}

/// A blur as the software's, that notes how large each picture it is
/// given is.
struct Measuring(Rc<std::cell::RefCell<Vec<Size<i32, Physical>>>>);

impl Frosting<PixmanRenderer, Drawn> for Measuring {
    fn blur(
        &mut self,
        renderer: &mut PixmanRenderer,
        picture: (&mut Drawn, Size<i32, Physical>),
        blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool {
        self.0.borrow_mut().push(picture.1);
        Frosting::<PixmanRenderer, Drawn>::blur(&mut Software, renderer, picture, blur, made)
    }

    fn element(
        &self,
        renderer: &mut PixmanRenderer,
        made: &dyn Any,
        at: (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<PixmanRenderer>>> {
        Frosting::<PixmanRenderer, Drawn>::element(&Software, renderer, made, at)
    }
}

#[test]
fn what_is_behind_glass_is_blurred_from_a_picture_half_as_large() {
    let mut session = Session::with_config(&[("theme.css", GLASS)]);
    let sizes = Rc::default();
    session.frosting = Some(Box::new(Measuring(Rc::clone(&sizes))));
    let mut window = session.connect();
    let _toplevel = session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    let picture = session.draw();
    let sizes = sizes.borrow();
    assert!(!sizes.is_empty(), "blurred");
    // The desktop behind it, as large as the screen, is pictured half as
    // large; and the glass is as red over the window as it was.
    let half = Size::<i32, Physical>::from(((WIDTH + 1) / 2, (HEIGHT + 1) / 2));
    assert!(
        sizes
            .iter()
            .all(|size| size.w <= half.w && size.h <= half.h),
        "{sizes:?}"
    );
    assert!(red(picture.at(100, 15)) > red(picture.at(400, 15)) + 64);
}

#[test]
fn glass_whose_blur_has_nothing_to_draw_is_solid_in_its_colour() {
    let (mut session, _window, _panel) = over_a_red_window(GLASS);
    session.frosting = Some(Box::new(Blank));
    session.step();
    let picture = session.draw();
    assert_eq!(picture.at(100, 15), 0xffff_ffff);
    assert_eq!(picture.at(400, 15), 0xffff_ffff);
}

#[test]
fn a_blur_that_blurs_as_the_software_does_is_kept_for_glass() {
    let mut session = Session::new();
    session.blurring();
    assert!(session.try_frosting(Ok(Software)).is_ok());
    assert_eq!(
        session.compositor().events().count(),
        0,
        "and nothing is said"
    );
}

#[test]
fn a_blur_that_blurs_otherwise_leaves_glass_solid_and_says_by_how_much() {
    let mut session = Session::new();
    session.blurring();
    let said = session.try_frosting(Ok(Wider)).err().unwrap();
    assert!(said.contains("Glass is drawn solid"), "{said}");
    assert!(said.contains("by as much as"), "{said}");
    let events: Vec<_> = session.compositor().events().collect();
    assert_eq!(events, [said.as_str()], "told once");
}

#[test]
fn a_blur_that_has_nothing_to_draw_leaves_glass_solid() {
    let mut session = Session::new();
    session.blurring();
    let said = session.try_frosting(Ok(Blank)).err().unwrap();
    assert!(said.contains("could not blur a picture"), "{said}");
}

#[test]
fn a_blur_that_could_not_be_made_leaves_glass_solid_and_says_why() {
    let mut session = Session::new();
    session.blurring();
    let said = session
        .try_frosting::<Software>(Err("no shader"))
        .err()
        .unwrap();
    assert!(said.contains("could not be made (no shader)"), "{said}");
    assert_eq!(session.compositor().events().count(), 1);
}

/// A theme in which a component's pop-up is the popover, glass white at
/// half over what is behind it blurred by 8 pixels, square and with no
/// edge, and `more`.
fn popover(more: &str) -> String {
    format!(
        "{GLASS}popover {{ background: rgba(255, 255, 255, 0.5); backdrop-filter: blur(8px); \
         box-shadow: none; border: none; border-radius: 0; }}\n{more}"
    )
}

/// A session with `theme`, a red window 200 by 100 in the top left corner,
/// the panel of [`over_a_red_window`], and a pop-up of the panel 100 by
/// 100 hanging from it at 150 across, so over the window's right edge,
/// tinted as the panel draws a popover and asking for nothing. The
/// window's program, the panel's, and the pop-up.
fn a_popover_over_a_red_window(theme: &str) -> (Session, [Client; 2], (WlSurface, XdgPopup)) {
    let mut session = Session::with_config(&[("theme.css", theme)]);
    session.blurring();
    let mut window = session.connect();
    session.open(&mut window, "app", (200, 100), RED);
    let mut panel = session.connect();
    panel.layer_name = "panel";
    let (_bar, role) = session.open_layer(&mut panel, &across(30), (WIDTH, 30), TINT);
    let (menu, popup) = panel.layer_popup(&role, (150, 30), (100, 100));
    session.roundtrip(&mut panel);
    panel.fill(&menu, (100, 100), TINT);
    session.roundtrip(&mut panel);
    session.roundtrip(&mut panel);
    (session, [window, panel], (menu, popup))
}

#[test]
fn a_pop_up_of_a_components_panel_is_glass_as_the_theme_has_the_popover() {
    let (mut session, _programs, _menu) = a_popover_over_a_red_window(&popover(""));
    let picture = session.draw();
    // Past the window's edge, under the pop-up: the window, blurred out.
    let (past, beside) = (picture.at(204, 60), picture.at(240, 120));
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
}

#[test]
fn a_pop_up_of_a_components_panel_is_not_glass_where_the_theme_says_none() {
    let theme = popover("popover { backdrop-filter: none; }\n");
    let (mut session, _programs, _menu) = a_popover_over_a_red_window(&theme);
    let picture = session.draw();
    assert_eq!(
        picture.at(204, 60),
        picture.at(240, 120),
        "the desktop, tinted"
    );
}

#[test]
fn a_pop_up_of_a_components_panel_casts_the_shadow_of_the_popover() {
    let theme = popover("popover { box-shadow: 0 8px 8px black; }\n");
    let (mut session, _programs, _menu) = a_popover_over_a_red_window(&theme);
    let picture = session.draw();
    // Just below it, over the desktop.
    let (shaded, clear) = (picture.at(240, 134), picture.at(400, 134));
    assert_ne!(shaded, clear, "a shadow below it");
}

#[test]
fn a_pop_up_of_a_components_panel_fading_out_keeps_its_glass_as_it_goes() {
    let theme = popover("popover { transition: 100ms linear; }\n");
    let (mut session, [_window, mut panel], (menu, popup)) = a_popover_over_a_red_window(&theme);
    session.advance(Duration::from_millis(100));
    drop(session.draw());
    popup.destroy();
    menu.destroy();
    session.roundtrip(&mut panel);
    session.step();
    session.advance(Duration::from_millis(50));
    let picture = session.draw();
    let (past, beside) = (picture.at(204, 60), picture.at(240, 120));
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
    // And is gone once it has.
    session.advance(Duration::from_millis(100));
    let picture = session.draw();
    assert_eq!(picture.at(240, 120), BACKGROUND);
}

#[test]
fn a_pop_up_of_a_components_panel_comes_in_as_large_as_the_popover_does_glass_and_all() {
    let theme = popover(
        "popover { transition: 100ms linear; }\n\
         @starting-style { popover { scale: 50%; } }\n",
    );
    let (mut session, _programs, _menu) = a_popover_over_a_red_window(&theme);
    session.advance(Duration::from_millis(50));
    let picture = session.draw();
    // Three quarters as large, about its middle, 200 across and 80 down:
    // its bottom right corner is not reached yet, its middle is, with what
    // is behind it blurred.
    assert_eq!(
        picture.at(247, 127),
        BACKGROUND,
        "not yet as far as its corner"
    );
    let (past, beside) = (picture.at(204, 60), picture.at(235, 110));
    assert!(red(past) > red(beside) + 8, "{past:#x} {beside:#x}");
}
