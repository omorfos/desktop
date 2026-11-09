//! The pointer of omorfos: every shape a program can ask for, drawn on a
//! grid of 24 as shapes and not as pictures, so that it is as sharp at any
//! size as at the one it was drawn for. In the dark scheme it is navy with a
//! rim of cream, and in the light one cream with a rim of navy, so that it
//! shows on any ground; what turns, the ring of waiting, is orange. Its
//! navy is navy-3, lighter than the ground's, which as small as a pointer
//! is cannot be told from black.
//!
//! The compositor draws it at the size it needs. Programs that read a
//! cursor theme from disk, as those written for X11 do, are given the same
//! shapes drawn at a few sizes, as the themes `omorfos-dark` and
//! `omorfos-light`.

use std::fmt::Write as _;

use crate::{CREAM, NAVY, NAVY_3, SPECTRUM, Scheme, render};

/// How many units across and down every shape is drawn in: a pointer 24
/// pixels large is drawn a pixel a unit.
const GRID: u32 = 24;

/// The width of the stroke under a shape that is its rim: half of it shows,
/// outside the shape.
const RIM: f32 = 2.4;

/// How far round the grid each picture reaches, in units, for the shadow
/// to be drawn whole.
const MARGIN: u32 = 3;

/// The shadow the pointer casts, so that it keeps an edge on a ground as
/// light as its rim: black at 35%, a unit down and blurred by 1.2.
const SHADOW: (&str, &str, &str) = ("0.35", "1", "1.2");

/// What turns: the quarter of the ring of waiting.
const TURNING: &str = SPECTRUM[3];

/// The sizes a theme on disk holds each shape at, of which a program takes
/// the nearest to the size it is told: those Adwaita's are drawn at.
pub const SIZES: [u32; 5] = [24, 32, 48, 64, 96];

/// A shape of the pointer, by what it says, as CSS and the cursor shape
/// protocol name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The arrow.
    Default,
    /// The arrow, with a menu.
    ContextMenu,
    /// The arrow, with a question.
    Help,
    /// A hand pointing, at what can be followed.
    Pointer,
    /// The arrow, with a ring: busy, but it can still be used.
    Progress,
    /// The ring alone: busy.
    Wait,
    /// A thick cross, for choosing cells.
    Cell,
    /// A thin cross, for choosing a point.
    Crosshair,
    /// A bar across lines of text.
    Text,
    /// A bar across text written down the page.
    VerticalText,
    /// The arrow, with an arrow bending: what is dropped is linked to.
    Alias,
    /// The arrow, with a plus: what is dropped is copied.
    Copy,
    /// Arrows four ways: what is under it can be moved.
    Move,
    /// The arrow, with a ring struck through: it cannot be dropped here.
    NoDrop,
    /// A ring struck through: not allowed.
    NotAllowed,
    /// An open hand: what is under it can be taken hold of.
    Grab,
    /// A closed hand: what is held is being moved.
    Grabbing,
    /// The right edge is moved.
    EResize,
    /// The top edge is moved.
    NResize,
    /// The top right corner is moved.
    NeResize,
    /// The top left corner is moved.
    NwResize,
    /// The bottom edge is moved.
    SResize,
    /// The bottom right corner is moved.
    SeResize,
    /// The bottom left corner is moved.
    SwResize,
    /// The left edge is moved.
    WResize,
    /// Left and right.
    EwResize,
    /// Up and down.
    NsResize,
    /// Up to the right and down to the left.
    NeswResize,
    /// Up to the left and down to the right.
    NwseResize,
    /// A line between columns is moved.
    ColResize,
    /// A line between rows is moved.
    RowResize,
    /// What is under it scrolls any way.
    AllScroll,
    /// A lens, with a plus.
    ZoomIn,
    /// A lens, with a minus.
    ZoomOut,
    /// The arrow, with a question in a box: what is dropped is asked about.
    DndAsk,
    /// Arrows four ways: any edge is moved.
    AllResize,
}

/// One part of a shape: what it is made of, under a rim of their own
/// together, and the marks drawn on it in the colour of the rim.
struct Layer {
    solids: String,
    marks: &'static str,
    /// How far its corners are rounded: the width of a stroke round it in
    /// its own colour, half of which shows, and which the rim is outside.
    softened: f32,
}

/// The arrow: a dart on the mark's angle, from its tip at the top left to
/// two wings and the notch between them, its corners rounded as the
/// mark's frame is.
const ARROW: &str = r#"<path d="M3.5 3.5 L19 10 L11.6 11.6 L10 19 Z"/>"#;
/// How much the arrow's corners are rounded.
const ARROW_SOFTENED: f32 = 1.6;
const BADGE: &str = r#"<circle cx="17.5" cy="17.5" r="5"/>"#;
const BOX: &str = r#"<rect x="12.5" y="12.5" width="10" height="10" rx="2"/>"#;
const QUESTION: &str = concat!(
    r#"<path d="M15.8 16.2 a1.8 1.8 0 1 1 2.5 1.7 q-0.8 0.35 -0.8 1.2" stroke-width="1.4"/>"#,
    r#"<circle cx="17.5" cy="21" r="0.8" fill="currentColor" stroke="none"/>"#,
);
const BEAM: &str = r#"<path d="M8 3 H16 V5 H13 V19 H16 V21 H8 V19 H11 V5 H8 Z"/>"#;
const BOTH_WAYS: &str =
    r#"<path d="M2 12 L7.5 6.5 V10 H16.5 V6.5 L22 12 L16.5 17.5 V14 H7.5 V17.5 Z"/>"#;
const FOUR_WAYS: &str = concat!(
    r#"<path d="M12 2 L16 6 H13.4 V10.6 H18 V8 L22 12 L18 16 V13.4 H13.4 V18 H16 L12 22 "#,
    r#"L8 18 H10.6 V13.4 H6 V16 L2 12 L6 8 V10.6 H10.6 V6 H8 Z"/>"#,
);
/// The top edge, and an arrow to it.
const TO_EDGE: &str = concat!(
    r#"<rect x="5" y="2.5" width="14" height="2.6" rx="1.3"/>"#,
    r#"<path d="M12 6.5 L17.5 12 H14 V21 H10 V12 H6.5 Z"/>"#,
);
/// The top left corner, and an arrow to it.
const TO_CORNER: &str = concat!(
    r#"<path d="M3 3 H12 V5.6 H5.6 V12 H3 Z"/>"#,
    r#"<path d="M7.5 7.5 L15.3 7.5 L13 9.8 L20 16.8 L16.8 20 L9.8 13 L7.5 15.3 Z"/>"#,
);
/// Two lines down the middle, and arrows away from them.
const APART: &str = concat!(
    r#"<rect x="9.4" y="3" width="2" height="18" rx="0.6"/>"#,
    r#"<rect x="12.6" y="3" width="2" height="18" rx="0.6"/>"#,
    r#"<path d="M2 12 L6.5 7.5 V10.5 H8.6 V13.5 H6.5 V16.5 Z"/>"#,
    r#"<path d="M22 12 L17.5 7.5 V10.5 H15.4 V13.5 H17.5 V16.5 Z"/>"#,
);
/// A palm, with the thumb, under fingers held up.
const OPEN_PALM: &str = concat!(
    r#"<path d="M6.5 12 H18.9 V16 Q18.9 21.5 13.4 21.5 H11 Q8.4 21.5 6.8 19.3 L3.6 14.8 "#,
    r#"Q2.8 13.5 4 12.8 Q5.1 12.2 6.1 13.1 L6.5 14.5 Z"/>"#,
);
const LENS: &str = concat!(
    r#"<circle cx="10" cy="10" r="7"/>"#,
    r#"<path d="M14 16.4 L16.4 14 L21.6 19.2 L19.2 21.6 Z"/>"#,
);

fn solid(solids: &str) -> Layer {
    marked(solids, "")
}

fn marked(solids: &str, marks: &'static str) -> Layer {
    Layer {
        solids: solids.to_owned(),
        marks,
        softened: 0.0,
    }
}

impl Shape {
    /// Every shape.
    pub const ALL: [Self; 36] = [
        Self::Default,
        Self::ContextMenu,
        Self::Help,
        Self::Pointer,
        Self::Progress,
        Self::Wait,
        Self::Cell,
        Self::Crosshair,
        Self::Text,
        Self::VerticalText,
        Self::Alias,
        Self::Copy,
        Self::Move,
        Self::NoDrop,
        Self::NotAllowed,
        Self::Grab,
        Self::Grabbing,
        Self::EResize,
        Self::NResize,
        Self::NeResize,
        Self::NwResize,
        Self::SResize,
        Self::SeResize,
        Self::SwResize,
        Self::WResize,
        Self::EwResize,
        Self::NsResize,
        Self::NeswResize,
        Self::NwseResize,
        Self::ColResize,
        Self::RowResize,
        Self::AllScroll,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::DndAsk,
        Self::AllResize,
    ];

    /// What CSS and the cursor shape protocol call it, which is also the
    /// name of its file in a cursor theme.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::ContextMenu => "context-menu",
            Self::Help => "help",
            Self::Pointer => "pointer",
            Self::Progress => "progress",
            Self::Wait => "wait",
            Self::Cell => "cell",
            Self::Crosshair => "crosshair",
            Self::Text => "text",
            Self::VerticalText => "vertical-text",
            Self::Alias => "alias",
            Self::Copy => "copy",
            Self::Move => "move",
            Self::NoDrop => "no-drop",
            Self::NotAllowed => "not-allowed",
            Self::Grab => "grab",
            Self::Grabbing => "grabbing",
            Self::EResize => "e-resize",
            Self::NResize => "n-resize",
            Self::NeResize => "ne-resize",
            Self::NwResize => "nw-resize",
            Self::SResize => "s-resize",
            Self::SeResize => "se-resize",
            Self::SwResize => "sw-resize",
            Self::WResize => "w-resize",
            Self::EwResize => "ew-resize",
            Self::NsResize => "ns-resize",
            Self::NeswResize => "nesw-resize",
            Self::NwseResize => "nwse-resize",
            Self::ColResize => "col-resize",
            Self::RowResize => "row-resize",
            Self::AllScroll => "all-scroll",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::DndAsk => "dnd-ask",
            Self::AllResize => "all-resize",
        }
    }

    /// The shape called `name`, if there is one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shape| shape.name() == name)
    }

    /// The point of the grid that is where the pointer is: the tip of the
    /// arrow or the finger, the middle of the lens, or the middle of all.
    const fn hotspot(self) -> (u32, u32) {
        // In tenths of a unit.
        match self {
            Self::Default
            | Self::ContextMenu
            | Self::Help
            | Self::Progress
            | Self::Alias
            | Self::Copy
            | Self::NoDrop
            | Self::DndAsk => (25, 25),
            Self::Pointer => (98, 22),
            Self::ZoomIn | Self::ZoomOut => (100, 100),
            _ => (120, 120),
        }
    }

    /// What it is made of, the parts over others last.
    fn layers(self) -> Vec<Layer> {
        std::iter::once(self.base()).chain(self.badge()).collect()
    }

    /// What is drawn first: the arrow, for the shapes that are the arrow
    /// saying more.
    fn base(self) -> Layer {
        match self {
            Self::Default
            | Self::ContextMenu
            | Self::Help
            | Self::Progress
            | Self::Alias
            | Self::Copy
            | Self::NoDrop
            | Self::DndAsk => Layer {
                softened: ARROW_SOFTENED,
                ..solid(ARROW)
            },
            Self::Pointer => marked(
                concat!(
                    r#"<rect x="8" y="2" width="3.6" height="13" rx="1.8"/>"#,
                    r#"<rect x="11.6" y="8" width="3.4" height="7" rx="1.7"/>"#,
                    r#"<rect x="15" y="9" width="3.2" height="6" rx="1.6"/>"#,
                    r#"<rect x="18.2" y="10.5" width="2.8" height="5" rx="1.4"/>"#,
                    r#"<path d="M8 12 H21 V16.5 Q21 21.5 16 21.5 H12.5 Q10 21.5 8.5 19.5 L4.2 14.6 "#,
                    r#"Q3.3 13.4 4.5 12.5 Q5.7 11.6 6.8 12.7 L8 14 Z"/>"#,
                ),
                r#"<path d="M11.6 9.8 V13 M15 10.8 V13.4 M18.2 12.2 V14" stroke-width="0.9"/>"#,
            ),
            Self::Wait => marked(
                concat!(
                    r#"<path fill-rule="evenodd" d="M3.5 12 a8.5 8.5 0 1 0 17 0 a8.5 8.5 0 1 0 -17 0 Z "#,
                    r#"M7.6 12 a4.4 4.4 0 1 0 8.8 0 a4.4 4.4 0 1 0 -8.8 0 Z"/>"#,
                ),
                r#"<path d="M12 5.55 a6.45 6.45 0 0 1 6.45 6.45" stroke="TURNING" stroke-width="4.1" stroke-linecap="butt"/>"#,
            ),
            Self::Cell => {
                solid(r#"<path d="M9.5 3 h5 v6.5 h6.5 v5 h-6.5 v6.5 h-5 v-6.5 h-6.5 v-5 h6.5 z"/>"#)
            }
            Self::Crosshair => solid(concat!(
                r#"<rect x="11.2" y="2" width="1.6" height="7.5" rx="0.8"/>"#,
                r#"<rect x="11.2" y="14.5" width="1.6" height="7.5" rx="0.8"/>"#,
                r#"<rect x="2" y="11.2" width="7.5" height="1.6" rx="0.8"/>"#,
                r#"<rect x="14.5" y="11.2" width="7.5" height="1.6" rx="0.8"/>"#,
                r#"<circle cx="12" cy="12" r="1"/>"#,
            )),
            Self::Text => solid(BEAM),
            Self::VerticalText => turned(90, BEAM),
            Self::Move | Self::AllResize => solid(FOUR_WAYS),
            Self::NotAllowed => solid(concat!(
                r#"<path fill-rule="evenodd" d="M3 12 a9 9 0 1 0 18 0 a9 9 0 1 0 -18 0 Z "#,
                r#"M6.2 12 a5.8 5.8 0 1 0 11.6 0 a5.8 5.8 0 1 0 -11.6 0 Z"/>"#,
                r#"<rect x="4" y="10.6" width="16" height="2.8" transform="rotate(45 12 12)"/>"#,
            )),
            Self::Grab => marked(
                concat!(
                    r#"<rect x="6.5" y="5" width="3" height="10" rx="1.5"/>"#,
                    r#"<rect x="9.7" y="3.5" width="3" height="11" rx="1.5"/>"#,
                    r#"<rect x="12.9" y="4.5" width="3" height="10" rx="1.5"/>"#,
                    r#"<rect x="16.1" y="6.5" width="2.8" height="8" rx="1.4"/>"#,
                ),
                r#"<path d="M9.6 7 V12.5 M12.8 6.5 V12.5 M16 8 V12.5" stroke-width="0.9"/>"#,
            )
            .with(OPEN_PALM),
            Self::Grabbing => marked(
                concat!(
                    r#"<rect x="6.5" y="8.5" width="3" height="6" rx="1.5"/>"#,
                    r#"<rect x="9.7" y="8" width="3" height="6" rx="1.5"/>"#,
                    r#"<rect x="12.9" y="8.3" width="3" height="6" rx="1.5"/>"#,
                    r#"<rect x="16.1" y="9" width="2.8" height="5" rx="1.4"/>"#,
                ),
                r#"<path d="M9.6 9.5 V12 M12.8 9.5 V12 M16 10 V12" stroke-width="0.9"/>"#,
            )
            .with(OPEN_PALM),
            Self::NResize => solid(TO_EDGE),
            Self::EResize => turned(90, TO_EDGE),
            Self::SResize => turned(180, TO_EDGE),
            Self::WResize => turned(270, TO_EDGE),
            Self::NwResize => solid(TO_CORNER),
            Self::NeResize => turned(90, TO_CORNER),
            Self::SeResize => turned(180, TO_CORNER),
            Self::SwResize => turned(270, TO_CORNER),
            Self::EwResize => solid(BOTH_WAYS),
            Self::NsResize => turned(90, BOTH_WAYS),
            Self::NeswResize => turned(-45, BOTH_WAYS),
            Self::NwseResize => turned(45, BOTH_WAYS),
            Self::ColResize => solid(APART),
            Self::RowResize => turned(90, APART),
            Self::AllScroll => solid(concat!(
                r#"<path d="M12 2 L15.5 6.5 H8.5 Z"/>"#,
                r#"<path d="M12 22 L15.5 17.5 H8.5 Z"/>"#,
                r#"<path d="M2 12 L6.5 8.5 V15.5 Z"/>"#,
                r#"<path d="M22 12 L17.5 8.5 V15.5 Z"/>"#,
                r#"<circle cx="12" cy="12" r="2.4"/>"#,
            )),
            Self::ZoomIn => marked(
                LENS,
                r#"<path d="M10 6.8 v6.4 M6.8 10 h6.4" stroke-width="1.6"/>"#,
            ),
            Self::ZoomOut => marked(LENS, r#"<path d="M6.8 10 h6.4" stroke-width="1.6"/>"#),
        }
    }

    /// What the arrow says more, at its lower right, if it does.
    fn badge(self) -> Option<Layer> {
        match self {
            Self::ContextMenu => Some(marked(
                BOX,
                r#"<path d="M15 15.3 h5 M15 17.5 h5 M15 19.7 h5" stroke-width="1.2"/>"#,
            )),
            Self::Help => Some(marked(BADGE, QUESTION)),
            Self::Progress => Some(marked(
                BADGE,
                concat!(
                    r#"<circle cx="17.5" cy="17.5" r="2.8" stroke-width="1.4"/>"#,
                    r##"<path d="M17.5 14.7 a2.8 2.8 0 0 1 2.8 2.8" stroke="TURNING" stroke-width="1.6" stroke-linecap="butt"/>"##,
                ),
            )),
            Self::Alias => Some(marked(
                BADGE,
                concat!(
                    r#"<path d="M15.2 19.8 q0 -3.6 3.6 -3.6" stroke-width="1.4"/>"#,
                    r#"<path d="M18 14.2 L20.6 16.2 L18 18.2 Z" fill="currentColor" stroke="none"/>"#,
                ),
            )),
            Self::Copy => Some(marked(
                BADGE,
                r#"<path d="M17.5 14.8 v5.4 M14.8 17.5 h5.4" stroke-width="1.6"/>"#,
            )),
            Self::NoDrop => Some(marked(
                BADGE,
                concat!(
                    r#"<circle cx="17.5" cy="17.5" r="2.9" stroke-width="1.3"/>"#,
                    r#"<path d="M15.45 15.45 L19.55 19.55" stroke-width="1.3"/>"#,
                ),
            )),
            Self::DndAsk => Some(marked(BOX, QUESTION)),
            _ => None,
        }
    }

    /// The shape as SVG, in the colours of `scheme`, to be drawn `pixels`
    /// large with `margin` pixels round it for its shadow.
    fn svg(self, scheme: Scheme, (pixels, margin): (u32, u32)) -> String {
        let (body, rim) = colours(scheme);
        let unit = f64::from(pixels) / f64::from(GRID);
        let start = -f64::from(margin) / unit;
        let side = f64::from(pixels + 2 * margin) / unit;
        let (opacity, down, blur) = SHADOW;
        let mut svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{start} {start} {side} {side}" width="{side}" height="{side}">"#,
                r#"<defs><filter id="shadow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="{blur}"/></filter></defs>"#,
                r#"<g filter="url(#shadow)" opacity="{opacity}" transform="translate(0 {down})">"#,
            ),
            start = start,
            side = side,
            blur = blur,
            opacity = opacity,
            down = down,
        );
        let layers = self.layers();
        for layer in &layers {
            let _ = write!(
                svg,
                r#"<g fill="black" stroke="black" stroke-width="{width}" stroke-linejoin="round">{solids}</g>"#,
                width = RIM + layer.softened,
                solids = layer.solids,
            );
        }
        svg.push_str("</g>");
        for layer in layers {
            let marks = layer.marks.replace("TURNING", TURNING);
            let _ = write!(
                svg,
                concat!(
                    r#"<g fill="{rim}" stroke="{rim}" stroke-width="{width}" stroke-linejoin="round">{solids}</g>"#,
                    r#"<g fill="{body}" stroke="{body}" stroke-width="{softened}" stroke-linejoin="round">{solids}</g>"#,
                    r#"<g color="{rim}" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round">{marks}</g>"#,
                ),
                rim = rim,
                width = RIM + layer.softened,
                softened = layer.softened,
                body = body,
                solids = layer.solids,
                marks = marks,
            );
        }
        svg.push_str("</svg>");
        svg
    }
}

impl Layer {
    /// The same, with `more` under the same rim as what it is made of.
    fn with(mut self, more: &str) -> Self {
        self.solids.push_str(more);
        self
    }
}

/// `solids` turned `degrees` clockwise about the middle of the grid.
fn turned(degrees: i32, solids: &str) -> Layer {
    let middle = GRID / 2;
    solid(&format!(
        r#"<g transform="rotate({degrees} {middle} {middle})">{solids}</g>"#
    ))
}

/// The colours of the inside of a shape and of its rim in `scheme`.
const fn colours(scheme: Scheme) -> (&'static str, &'static str) {
    match scheme {
        Scheme::Dark => (NAVY_3, CREAM),
        Scheme::Light => (CREAM, NAVY),
    }
}

/// The names X11 and older programs ask for shapes by, which a cursor theme
/// has as links to the shapes' own files: those the `cursor-icon` crate
/// gives, but for two that are the names of other shapes, `copy` and
/// `move`, and with each that it gives for two shapes, as
/// `h_double_arrow`, given to the first.
pub const OLDER: [(&str, Shape); 45] = [
    ("left_ptr", Shape::Default),
    ("arrow", Shape::Default),
    ("top_left_arrow", Shape::Default),
    ("left_arrow", Shape::Default),
    ("question_arrow", Shape::Help),
    ("whats_this", Shape::Help),
    ("hand2", Shape::Pointer),
    ("hand1", Shape::Pointer),
    ("hand", Shape::Pointer),
    ("pointing_hand", Shape::Pointer),
    ("left_ptr_watch", Shape::Progress),
    ("half-busy", Shape::Progress),
    ("watch", Shape::Wait),
    ("plus", Shape::Cell),
    ("cross", Shape::Crosshair),
    ("xterm", Shape::Text),
    ("ibeam", Shape::Text),
    ("link", Shape::Alias),
    ("circle", Shape::NoDrop),
    ("crossed_circle", Shape::NotAllowed),
    ("forbidden", Shape::NotAllowed),
    ("openhand", Shape::Grab),
    ("fleur", Shape::Grab),
    ("closedhand", Shape::Grabbing),
    ("right_side", Shape::EResize),
    ("top_side", Shape::NResize),
    ("top_right_corner", Shape::NeResize),
    ("top_left_corner", Shape::NwResize),
    ("bottom_side", Shape::SResize),
    ("bottom_right_corner", Shape::SeResize),
    ("bottom_left_corner", Shape::SwResize),
    ("left_side", Shape::WResize),
    ("h_double_arrow", Shape::EwResize),
    ("size_hor", Shape::EwResize),
    ("v_double_arrow", Shape::NsResize),
    ("size_ver", Shape::NsResize),
    ("fd_double_arrow", Shape::NeswResize),
    ("size_bdiag", Shape::NeswResize),
    ("bd_double_arrow", Shape::NwseResize),
    ("size_fdiag", Shape::NwseResize),
    ("split_h", Shape::ColResize),
    ("sb_h_double_arrow", Shape::ColResize),
    ("split_v", Shape::RowResize),
    ("sb_v_double_arrow", Shape::RowResize),
    ("size_all", Shape::AllScroll),
];

/// What the cursor theme of the pointer in `scheme` is called: the
/// directory it is installed as.
#[must_use]
pub fn theme(scheme: Scheme) -> String {
    format!("omorfos-{}", scheme.name())
}

/// What says that the directory of [`theme`] is a cursor theme.
#[must_use]
pub fn index(scheme: Scheme) -> String {
    let scheme = scheme.name();
    format!(
        "[Icon Theme]\nName=omorfos {scheme}\nComment=The pointer of omorfos, for the {scheme} scheme\n"
    )
}

/// A shape of the pointer drawn at one size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    /// The pixels, row by row from the top, each blue, green, red and
    /// opacity, with the colours already multiplied by the opacity: what
    /// renderers call `Argb8888`, and what a cursor file holds.
    pub pixels: Vec<u8>,
    /// How large it is: how many pixels across and down its grid is.
    pub size: u32,
    /// How many pixels across and down the picture is: its grid, and the
    /// room round it its shadow is cast in.
    pub across: u32,
    /// The pixel that is where the pointer is.
    pub hotspot: (u32, u32),
}

/// `shape` in the colours of `scheme`, `pixels` large, with its shadow
/// round it; or nothing, at no size.
#[must_use]
pub fn pointer(shape: Shape, scheme: Scheme, pixels: u32) -> Option<Drawn> {
    let margin = (MARGIN * pixels).div_ceil(GRID);
    let across = pixels + 2 * margin;
    let canvas = render(&shape.svg(scheme, (pixels, margin)), (across, across))?;
    let mut bytes = canvas.take();
    // tiny-skia's red, green, blue and opacity, as blue, green, red and
    // opacity.
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let (across_tip, down_tip) = shape.hotspot();
    let at = |tenths: u32| margin + (tenths * pixels / (GRID * 10)).min(pixels.saturating_sub(1));
    Some(Drawn {
        pixels: bytes,
        size: pixels,
        across,
        hotspot: (at(across_tip), at(down_tip)),
    })
}

/// The file of a cursor theme for `shape` in `scheme`, holding it at each of
/// [`SIZES`], in the format Xcursor reads.
#[must_use]
pub fn theme_file(shape: Shape, scheme: Scheme) -> Vec<u8> {
    let drawn = SIZES
        .iter()
        .filter_map(|&size| pointer(shape, scheme, size));
    xcursor(&drawn.collect::<Vec<_>>())
}

/// `images` as a cursor file: a header, a table of where each image is,
/// and the images, each with a header of its own; every number four bytes,
/// the least significant first.
fn xcursor(images: &[Drawn]) -> Vec<u8> {
    /// What a chunk of the file holding an image is marked as.
    const IMAGE: u32 = 0xfffd_0002;
    const HEADER: u32 = 16;
    const IMAGE_HEADER: u32 = 36;
    let count = u32::try_from(images.len()).unwrap_or(u32::MAX);
    let words = |words: &[u32]| {
        words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    };
    let mut file = b"Xcur".to_vec();
    file.extend(words(&[HEADER, 0x1_0000, count]));
    let mut at = HEADER + 12 * count;
    for image in images {
        file.extend(words(&[IMAGE, image.size, at]));
        at += IMAGE_HEADER + 4 * image.across * image.across;
    }
    for image in images {
        let (x, y) = image.hotspot;
        let (size, across) = (image.size, image.across);
        // Its version, then no delay, as it does not change.
        file.extend(words(&[
            IMAGE_HEADER,
            IMAGE,
            size,
            1,
            across,
            across,
            x,
            y,
            0,
        ]));
        file.extend(&image.pixels);
    }
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The opacity of the pixel at `(x, y)`.
    fn opacity(drawn: &Drawn, (x, y): (u32, u32)) -> u8 {
        let at = usize::try_from((y * drawn.across + x) * 4 + 3).unwrap();
        drawn.pixels[at]
    }

    #[test]
    fn every_shape_is_drawn_at_any_size_with_its_hotspot_on_it() {
        for scheme in [Scheme::Dark, Scheme::Light] {
            for shape in Shape::ALL {
                for size in [16, 24, 30, 48, 100] {
                    let drawn = pointer(shape, scheme, size).unwrap();
                    assert_eq!(drawn.size, size);
                    // An eighth of it round each side, for the shadow.
                    let margin = (size * 3).div_ceil(24);
                    assert_eq!(drawn.across, size + 2 * margin);
                    let across = usize::try_from(drawn.across).unwrap();
                    assert_eq!(drawn.pixels.len(), across * across * 4);
                    let tip = drawn.hotspot;
                    let on_grid = |at: u32| (margin..margin + size).contains(&at);
                    assert!(on_grid(tip.0) && on_grid(tip.1), "{shape:?} {size}");
                    // The ring of waiting is where it is from its middle,
                    // which its shadow falls in.
                    assert!(opacity(&drawn, tip) > 0, "{shape:?} {size}: {tip:?}");
                }
            }
        }
    }

    #[test]
    fn the_arrow_points_from_the_top_left_and_its_hotspot_is_its_tip() {
        // Its grid begins 6 pixels in, round which its shadow is cast.
        let drawn = pointer(Shape::Default, Scheme::Dark, 48).unwrap();
        assert_eq!(drawn.hotspot, (11, 11));
        assert_eq!(opacity(&drawn, (1, 1)), 0, "nothing above and left of it");
        assert_eq!(opacity(&drawn, (46, 10)), 0, "nor to its right");
        assert_eq!(
            opacity(&drawn, (24, 24)),
            255,
            "its body, along the mark's angle"
        );
    }

    #[test]
    fn it_is_navy_rimmed_with_cream_when_dark_and_cream_rimmed_with_navy_when_light() {
        // Blue, green and red of the pixel at `at`.
        let colour = |drawn: &Drawn, (x, y): (u32, u32)| {
            let at = usize::try_from((y * drawn.across + x) * 4).unwrap();
            [drawn.pixels[at + 2], drawn.pixels[at + 1], drawn.pixels[at]]
        };
        let (navy, navy_3, cream) = ([0x0c, 0x12, 0x26], [0x24, 0x33, 0x64], [0xfc, 0xf7, 0xe3]);
        let dark = pointer(Shape::Default, Scheme::Dark, 96).unwrap();
        let light = pointer(Shape::Default, Scheme::Light, 96).unwrap();
        // Inside the arrow, along the mark's angle from its tip, and on the
        // rim of its left edge, its grid beginning 12 pixels in.
        let (inside, rim) = ((48, 48), (31, 52));
        assert_eq!(colour(&dark, inside), navy_3);
        assert_eq!(colour(&dark, rim), cream);
        assert_eq!(colour(&light, inside), cream);
        assert_eq!(colour(&light, rim), navy);
    }

    #[test]
    fn it_casts_a_soft_black_shadow_down_and_hardly_up() {
        let drawn = pointer(Shape::Default, Scheme::Light, 96).unwrap();
        let pixel = |(x, y): (u32, u32)| {
            let at = usize::try_from((y * drawn.across + x) * 4).unwrap();
            [0, 1, 2, 3].map(|channel| drawn.pixels[at + channel])
        };
        // Just under the rim of its lower wing, and as far over its tip.
        let (under, over) = ((52, 96), (26, 16));
        let [blue, green, red, opacity] = pixel(under);
        assert_eq!([blue, green, red], [0, 0, 0], "black");
        assert!(opacity > 30 && opacity < 90, "and soft: {opacity}");
        assert!(pixel(over)[3] < opacity / 4, "fainter over it");
    }

    #[test]
    fn every_shape_is_its_own_but_for_the_two_that_are_the_same_arrows() {
        let drawn: Vec<_> = Shape::ALL
            .map(|shape| pointer(shape, Scheme::Dark, 24).unwrap().pixels)
            .to_vec();
        for (at, (one, first)) in drawn.iter().zip(Shape::ALL).enumerate() {
            for (other, shape) in drawn[at + 1..].iter().zip(&Shape::ALL[at + 1..]) {
                let same = [first, *shape] == [Shape::Move, Shape::AllResize];
                assert_eq!(one == other, same, "{first:?} and {shape:?}");
            }
        }
    }

    #[test]
    fn a_shape_is_found_by_its_name_and_nothing_by_another() {
        for shape in Shape::ALL {
            assert_eq!(Shape::named(shape.name()), Some(shape));
        }
        assert_eq!(Shape::named("pirate"), None);
    }

    #[test]
    fn nothing_is_drawn_at_no_size() {
        assert_eq!(pointer(Shape::Default, Scheme::Dark, 0), None);
    }

    #[test]
    fn a_theme_file_holds_the_shape_at_each_size_with_its_hotspot() {
        let file = theme_file(Shape::Pointer, Scheme::Light);
        let images = xcursor::parser::parse_xcursor(&file).unwrap();
        let sizes: Vec<u32> = images.iter().map(|image| image.size).collect();
        assert_eq!(sizes, SIZES);
        for image in images {
            let drawn = pointer(Shape::Pointer, Scheme::Light, image.size).unwrap();
            assert_eq!((image.width, image.height), (drawn.across, drawn.across));
            assert_eq!((image.xhot, image.yhot), drawn.hotspot);
            assert_eq!(image.pixels_rgba, drawn.pixels, "as the file holds them");
        }
    }
}
