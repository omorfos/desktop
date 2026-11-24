//! The signs of the interface: small shapes in the colour of the text
//! beside them, for an action or a state, as `docs/DESIGN.md` has them.
//! Here they are only named, for what they stand for; how each is drawn
//! is not the model's to know.

/// One of the signs the interface is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sign {
    /// A cross: to close.
    Close,
    /// A line: to minimize.
    Minimize,
    /// Four corners: to fill the screen.
    Fullscreen,
    /// Tiles: to put a window beside others.
    Tile,
    /// The left half marked.
    LeftHalf,
    /// The right half marked.
    RightHalf,
    /// The top half marked.
    TopHalf,
    /// The bottom half marked.
    BottomHalf,
    /// An arrow to a line: to send somewhere else.
    Send,
    /// A plus: to add.
    Add,
    /// Arrows outwards: to make as large as there is room for.
    Expand,
    /// A window with its top closed: to roll up into the titlebar.
    Shade,
    /// A pin: to keep above the others.
    Pin,
    /// An eye in a frame: to show everywhere.
    Everywhere,
    /// A cup of coffee: to stay awake.
    Awake,
    /// A magnifying glass: to look for something.
    Search,
    /// Two arrows round: to read again.
    Reload,
    /// An "i" in a ring: to say what something is.
    About,
    /// An arrow out of a door: to leave.
    Quit,
    /// A padlock: to lock.
    Lock,
    /// A moon: to sleep.
    Sleep,
    /// An arrow turning back: to start again.
    Restart,
    /// A switch's sign: to turn off.
    ShutDown,
    /// Four squares: the Super key.
    Super,
    /// A caret up: the Ctrl key.
    Ctrl,
    /// The option sign: the Alt key.
    Alt,
    /// A broad arrow up: the Shift key.
    Shift,
    /// An arrow down and back: the Enter key.
    Enter,
    /// A bracket lying down: the space bar.
    Space,
    /// An arrow to a line: the Tab key.
    Tab,
    /// An arrow up: the key of that arrow.
    Up,
    /// An arrow down: the key of that arrow.
    Down,
    /// An arrow left: the key of that arrow.
    Left,
    /// An arrow right: the key of that arrow.
    Right,
    /// A key with a cross: the Backspace key.
    Backspace,
    /// A globe: the web browser.
    Browser,
    /// A folder: what manages files.
    Files,
    /// A pen on a page: the text editor.
    Editor,
    /// A prompt and a line: the terminal.
    Terminal,
    /// A cog: the settings.
    Settings,
}
