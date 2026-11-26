# Roadmap

What omorfos is to become, in the order it is to be built. Stages are
worked top to bottom and one at a time; within a stage the order is a
suggestion. Tick items as they land and keep the "Next" pointer current.

**Next:** what closes stage 4, decided on 24 November 2026 and to be
built: the keys and gestures of ADR 101 as accepted, and the rest of the
redesign of windows (ADR 97, ADR 107). Stage 5, the bar and the dock, follows, wearing that look.
What else is left of stages 3 and 4 is known gaps, taken as the code
beside them is touched. Of what is taken from stage 8 ahead of its turn,
the first release waits, there being no hurry for it, as do packages
beyond `.deb` and the AUR.

Items that came from the wishlist kept outside this repository carry its
number, as (wish 3), so that nothing asked for is lost track of.

## To be tried by hand

What has passed the tests but has not yet been tried on real hardware by
the maintainer. An agent starting work reminds them of what is here, and
an item goes once it has been tried: what was wrong goes to its stage as
a known gap.

- **A password box withdrawn keeps what is typed (#180, finding 26).**
  In a terminal, `pkexec true`, and once the box is up type a few
  letters; from another terminal end the `pkexec`. The box goes, and
  what you go on typing, and Enter, reach nothing until you stop for
  half a second, or click; then the terminal has the keyboard, with
  nothing typed into it. Answered with Escape, the box gives the
  keyboard back at once.
- **Time limits put off by being there (#180, finding 7; ADR 112).**
  Shut Down from the root menu, and move the mouse now and then: it does
  not go ahead while you do; left alone a minute, it does. With
  `power: { wait: 0 }` it says no seconds and waits. Press the display
  key with two screens: Put back and Keep are there, Escape puts the
  screens back at once, and moving the mouse puts the 15 seconds off, but
  they go back by two minutes. Rest the pointer on a notification: it
  stays, and goes five seconds after the pointer leaves. With `mouse:
  { double_click: 800 }`, two slow clicks on a titlebar maximize it. On
  the lock screen, awake, moving the mouse keeps the field.
- **Questions answered only by what was meant for them (#180,
  finding 3; ADR 111).** In a browser, start sharing the screen while
  typing in a text field, and press Enter as the question comes up: it
  is cancelled, nothing shared, the keyboard's ring on Cancel. Asked
  again, Tab to Share and Enter at once does nothing; a moment later it
  shares. Press on Share, drag off it and let go: nothing. Shut Down
  from the root menu: Enter cancels, Tab and Enter after a moment goes
  on.
- **The theme's elements in a tree (ADR 110, step 1).** Everything
  looks as it did. Then in `~/.config/omorfos/theme.css`: an old name,
  as `launcher-mode-key { display: none; }`, is refused, the notice
  saying `launcher > modes > button > kbd`, and the theme kept as it
  was; `button { border-radius: 2px; }` squares every button, the
  titlebar's, the launcher's modes, a question's and the overview's
  close buttons among them; `launcher > modes > button > kbd { display:
  none; }` takes the keys out of the row of modes; and `launcher {
  modes { button:hover { background: red; } } }`, nested, reddens a
  mode under the pointer.
- **The program's windows alone (#141).** With two windows of one
  program and one of another on a workspace, the first focused,
  Super+Ctrl+Down opens the overview with the two alone, the one in
  front picked; Super+Ctrl+Down again closes it, and so does it with
  the whole overview open. With no window in front it does nothing.
- **Down gives the programs the keyboard (#166).** In the launcher,
  type `f`: no program is lit, and Enter would start the first. Down
  lights the first, Files, rather than the one after it, and the field's
  cursor goes; Left and Right walk the programs, in the list and in the
  grid of every program alike. The pointer over a program does the
  same. Typing, a click in the field, or Up past the first program gives
  the field the keyboard back, its cursor with it, and nothing lit.
- **The cursor blinks (#167).** In the launcher, the lock screen's field
  and the box that asks for a password, the cursor blinks, half a
  second on and half off, from each key, and stays shown ten seconds
  after the last. With `appearance.reduce_motion`, it does not blink.

- **Super with a digit in the launcher (ADR 109, part 1).** The rest
  was tried; this needs Super, which WSL keeps. With the launcher open,
  `Super+1` goes to the grid, `Super+2` does not change the workspace,
  and `Super+Shift+3` still takes a picture of the screen.
- **Room round the titlebar buttons' signs.** With the pointer on a
  titlebar's buttons, the cross, bar and plus are smaller than the
  buttons' half, with room all round them, the cross's ends clear of
  the edge; at `output: { scale: 1.25 }` too.

- **The launcher's look and motion (ADR 109, part 2).** `Super+Space`:
  a large field with a magnifying glass and "Search" in it, nothing
  under it, as much room under the field as over it, and the launcher
  comes in from a little smaller. Type `fi`: it grows to the top hit,
  outlined under "Top hit", and the rest under "Programs"; `zzzq`, one
  dim line. Down lights the top hit where it is, and Down again slides
  the ground to Firefox, past the heading. With nothing typed, the
  pointer over the launcher shows the row of modes at its bottom;
  clicking Programs, or `Super+1`, turns the sign into four squares,
  the launcher widens and grows to the grid and the tiles fade in, with
  the field where it was. Escape closes it a little quicker than it
  came. With `appearance.reduce_motion`, all of it at once. Then with
  `fi` typed, Down past the last program: no row lit, the row of modes
  with a ring round All; Right moves the ring to Programs, Enter goes
  there with `fi` still typed; Up from the ring goes back to the last
  program, Escape to the field.

## Where this is going

omorfos is small, fast and driven by text files, but what is being asked
of it now is a whole desktop: a bar, a dock, a way to switch windows,
settings, and programs to start with. It should look modern, in
the frosted-glass manner, and stay simple, clean, accessible and above all
secure (wish 10).

The priorities in [AGENTS.md](../AGENTS.md) do not change, and they
settle what comes first here: security, then correctness, then
performance, then everything else. Three things follow from them.

- **The compositor stays small.** What can be a program of its own is one
  (ADR 18), so that a fault in a dock is not a fault in the session.
- **Nothing is required that can be optional.** The applications in stage
  7 come with omorfos and are not needed by it. Someone who prefers
  another file manager, panel or notification program can have it.
- **Looks are built once.** The design language and how effects are drawn
  are decided before the bar and the dock are written, not after.

And one thing beside them, which is what omorfos is for as much as being
small and safe is (ADR 99): **the standards there are, before any of its
own.** It speaks what desktops have agreed; where they have agreed
nothing it says so ([STANDARDS.md](STANDARDS.md)) and takes it to those
who could, so that the desktops are easier to be beside, for its users
and for theirs. It is not to be one more way of doing what there are
already too many of.

## How the stages were ordered

1. What takes a day and is felt every day comes first.
2. Decisions that many later things rest on are made before those things,
   each as an ADR, and cost little.
3. What stops omorfos being used all day comes before what makes it
   pleasant: sharing a screen in a call matters more than a dock.
4. The look comes before the shell that is to wear it.
5. Applications come last, because they are the most work, the least
   bound to omorfos, and need the toolkit decision of stage 2.

Five things change that order where it would have cost more than it
saves: four found by a review of the roadmap on 2026-10-25, and a fifth
decided the day after:

- **A known gap does not hold its stage open.** An item that begins
  "Known gaps" is written down so that it is not forgotten, and is taken
  when the code beside it is touched. None stops omorfos being used all
  day.
- **Some of stage 8 comes straight after stage 3**, being cheap and of
  use at once: `omorfosctl report`, which every report from real
  hardware has wanted; the parsers fuzzed, for security; the links
  between documents checked; what a version promises, and CI that finds
  a breaking change not marked as one; and the first release, which is
  the first real run of the release pipeline. Each stays in stage 8,
  where it is described, and is taken first.
- **In stage 4, `appearance` and the redrawn theme come before glass**:
  more waits on them (the portal's settings, `reduce_motion`, `scheme:
  auto`) than on the blur.
- **What waits on another project says so** ("Waits on"), and is not
  started until that has changed.
- **What lets the compositor be started again comes early** (ADR 60):
  the holder at the end of stage 3, since it ends what ADR 59 began and
  makes trying each later change on real hardware a matter of installing
  it; and the relay straight after it, ahead of the rest of stage 3, since
  until it is there a compositor started again closes every window but
  Qt 6's, and nothing else asked for spares the user as much.

## 1. Quick wins

Small, independent, and each an improvement the day it lands.

- [x] The launcher on `Super+Space` (wish 2). The root menu, which had
      that key, is on `Super+Escape` until the bar has its menu.
- [x] Mouse and touchpad set apart (wish 4): the touchpad scrolls the
      natural way by default and the mouse does not, each with its own
      setting.
- [x] Pointer speed and acceleration, for the mouse and for the touchpad.
- [x] Titlebar buttons: close, minimize, maximize, coloured, each a part
      of the theme (wish 3), at the right since ADR 68 and at the left
      since ADR 97. Double-click on a titlebar maximizes.
- [x] A menu on a right click of the titlebar (wish 3): the same three,
      and shade (roll the window up into its titlebar), keep above the
      others, show on every workspace, send to a workspace (ADR 31). The
      first three of those are also actions, to bind keys to: `shade`,
      `keep_above` and `stick`.
- [x] What a window is kept as, told to panels and taskbars over IPC and
      set by rules: shaded, above, on every workspace.
- [x] The root menu, and the titlebar's, driven from the keyboard:
      arrows, Enter, Escape.
- [x] The work pushed and seen to pass in CI, on amd64 and arm64, the
      XWayland tests with it.

## 2. Decisions

Each is an ADR, written and agreed before the code that depends on it.
Most are a day of thought and no code.

- [x] **The design language** (wish 10), in `docs/DESIGN.md` (ADR 32):
      Inter and JetBrains Mono, sizes and spacing, radii, a dark and a
      light palette with one accent and the contrast measured, glass with
      a floor to how see-through it may be, what moves and how, signs
      from Lucide, and reduced motion, reduced transparency and high
      contrast as settings from the start. What is someone else's is
      credited in `CREDITS.md`.
- [x] **How effects are drawn** (ADR 33): blur is a shader, in one file
      left out of coverage, checked at every start against a software
      blur that is tested and is its specification; everything that is a
      decision is outside that file. Shadows are pictures made on the
      CPU. Nothing is blurred again until what is under it changes.
- [x] **What components and applications are written with** (ADR 34):
      one toolkit, ours, `omorfos-ui` grown by need into one, for the
      shell and for the applications. Text editing comes from the text
      library already used, and speaking to screen readers from
      AccessKit. Two trials guard it: a long list scrolling on a dense
      screen, tried with the first list written, and the file manager,
      which comes last and may fall back to a toolkit that exists.
- [x] **Where omorfos meets the rest of the system** (ADR 35): the
      compositor speaks to no service. `omorfos-session` speaks to
      logind and systemd, `omorfos-lock` to PAM, `omorfos-ask` to polkit
      and for pinentry, `xdg-desktop-portal-omorfos` for the portals,
      and the bar to UPower, NetworkManager and the tray. Every one is
      optional, none runs as root, and a locked session is opened only
      by its user's secret.
- [x] **Workspaces that come and go** (wish 1, ADR 36): desktops made
      when wanted and removed when empty and left, one by default; and a
      solo workspace for each fullscreen window, as macOS has it, gone
      when the window closes or leaves fullscreen. The titlebar's second
      button means fullscreen. In a solo, pushing the pointer against
      the top brings down the bar and the titlebar, and against the
      bottom the dock.
- [x] **Who may do what** (ADR 37): a privilege is given to a program
      the compositor started itself, for a role the configuration names
      (`may: [panel]`), never by a name a program can choose. A program
      in a sandbox gets none and asks the portals, which ask the user.
      A mark while the screen is captured, one locker, and a prompt for
      secrets that cannot be imitated are the compositor's and no role
      alters them.
- [x] **Pictures of windows** (ADR 38): they never leave the
      compositor. It draws the switcher and the overview itself, and
      lends the dock a window's picture by drawing it in a rectangle the
      dock leaves empty, live and made small by halves. A rule can mark
      a window `private`, shown as its icon and left blank in captures.

## 3. What a day's use needs

Without these, omorfos is put aside for something else when the day
calls for them.

- [x] Workspaces as ADR 36 has them, in the model first: identities,
      desktops made and removed, `new_workspace`, the number keys making
      the next one, one by default, and what the IPC says of them.
- [x] Fullscreen on a workspace of its own: the solo, the window going
      back where it was, and the titlebar's button meaning fullscreen.
- [x] In a fullscreen workspace, the panels and the titlebar brought
      back by resting the pointer against an edge (ADR 36).
- [x] The same by pushing against the edge, without the wait, on a real
      mouse or touchpad.
- [x] Not at all while a program holds the pointer (ADR 57).
- [x] Tiled workspaces (ADR 39): a workspace made for up to four
      windows that fill the screen between them. First the model, with
      two windows side by side made from the titlebar's menu and by a
      key, each told it is tiled, and a window that leaves giving its
      room up.
- [x] What is brought back at an edge staying while the pointer moves,
      so that a titlebar away from the edge can be reached.
- [x] The lines between tiles drawn and dragged.
- [x] A line that stops where a window says it can be no smaller, and a
      part of the theme for the line.
- [x] A window that says it can be no smaller than its part, when the
      part is made or the window says so later, given more room without
      the line being dragged.
- [x] Three and four windows, changing places and focus by direction.
- [x] A window changed over with another by dragging its titlebar onto
      the other's part.
- [x] Windows that share a workspace keep their titlebars; only the
      panels hide (ADR 39, "Titlebars after all").
- [x] An empty part of a tiled workspace filled by the next window to
      open there.
- [x] A window that is open already put beside one that is tiled or
      fullscreen, without opening it there.
- [x] Pictures of the arrangements at the titlebar's second button,
      when the pointer rests on it.
- [x] A window dragged to an edge of the screen put in a tiled
      workspace, with the half it would take shown before it is let go.
- [x] Choosing a window for an empty part from those on the desktop,
      shown small (ADR 38), made small in one step for now.
- [x] Titles under the windows offered in an empty part, which a
      private one, shown blank, needs most.
- [x] A window offered in an empty part as one box, its picture and its
      title: washed over whole, chosen by a click on either, with a faint
      border, which shows where a private window's blank is, and a state
      of the `offer` part for the title under the pointer; the wash shown
      over a private window's blank, under which it is hidden now; and the
      titles of a row on one line, under the tallest of its pictures.
- [x] Icons beside the titles of windows offered in an empty part, and
      large in the middle of a private window's blank (ADR 38): the one
      its program gives, or its application's, as the switcher has them.
- [ ] Windows made small by halves (ADR 38), once the renderer for
      real hardware can draw from what it drew: before the switcher.
- [x] A window that nobody asked for, opening while a fullscreen
      workspace is shown, left on the desktop unseen (activation
      tokens).
- [ ] A link clicked in VS Code opened its page in Firefox without
      Firefox coming in front, found by hand on 17 November 2026, where
      links in other programs and files from a file manager do bring
      theirs in front. VS Code was running as a Wayland program. Whether
      it asked for a token, handed it on, or was refused is not known:
      what it says to the compositor (`WAYLAND_DEBUG=1`) is to be read.
- [x] Roles (ADR 37): components started by being handed their
      connection, `may` on what `autostart` and `spawn` start, panels,
      taking the keyboard and locking each behind a check of the role,
      the `lock` action and `locker`, and the user told what was refused
      and how to allow it.
- [x] Programs in a sandbox recognised (`security-context-v1`) and shown
      nothing they may not have: not panels, locking, the list of
      windows, or the way a sandbox is made.
- [x] Pictures of a screen (`ext-image-copy-capture-v1`), for a program
      with the `capture` role, with a mark on the screen while they are
      taken that is not in them. `omorfos-screenshot` saves one of every
      screen, and `Print` starts it. It is written through the file
      behind the program's buffer, without `unsafe` (ADR 61).
- [x] A picture put on the clipboard instead of, or as well as, saved:
      `screenshots.destination`, `--to`, and `Shift+Print`. With it the
      `clipboard` role and `ext-data-control-v1`, since a program
      without a window cannot otherwise set the clipboard.
- [x] A clipboard that outlives the program that copied: a manager of
      our own with the `clipboard` role, which would also let
      `omorfos-screenshot` stop at once (ADR 42).
- [ ] What `omorfos-clipboard` keeps, offered as a history to choose
      from: the clipboard with a history of stage 6, which is the same
      work and is taken there.
- [x] Forgetting what was copied after a while, for passwords copied
      from where nothing says they are secret: Proton Pass in Firefox
      clears what it copied only while its popup stays open.
      `clipboard.forget` empties the clipboard that many seconds after
      what the keeper kept, unless something else was copied since.
- [x] The `private` rule, which leaves a window blank in any picture
      taken of the screen, for as long as it matches (ADR 38).
- [x] Pictures of one window (`ext-foreign-toplevel-image-capture-source`),
      and the screenshot tool taking one of the window with focus, with
      `--window` and `Alt+Print`.
- [x] Pictures of a window as large as it is, and see-through where it
      is, saved so by `omorfos-screenshot --window`.
- [ ] Known gaps in pictures of a window: it is given in the frame
      omorfos draws, with no way to ask for it without; one taken in
      the memory of the graphics card is solid, see-through only in
      shared memory; and a titlebar of glass is in it as its tint alone,
      with nothing of what is behind it, which is not in the picture.
- [x] A private window blurred in pictures in place of blank, as a
      choice of the rule (`private: blur`): the whole frame under glass
      that blurs it, blank where glass is solid. A blur hides less than a
      blank does, which the guide says (ADR 38).
- [ ] Known gaps in a blurred private window: a picture of it by itself
      is blank, being drawn without glass; and how far it is blurred is
      not the theme's to say.
- [x] A private window's last moment, as it fades when closed, left out
      of pictures as the window was.
- [x] The menus of a program written for X11 with a private window left
      out of pictures with it, the program known by the process it says
      each window is of (`_NET_WM_PID`).
- [ ] A coverage gap that comes by timing, seen once on 15 November 2026
      in the gates without a graphics card and not on the run after: a
      picture PipeWire has taken away while it was held
      (`omorfos-pipewire`'s `Held::drop`, and the portal's `held.gone()`
      in `xdg-desktop-portal-omorfos`), which only some runs reach. The
      tests are to bring it about for certain.
- [ ] The portal's question is said in English whatever the user's
      language, its words in the code rather than messages, and does not
      light the button under the pointer, as it is not drawn again as the
      pointer moves.
- [ ] Known gaps in what is private: a notification about a private
      window is in a picture, as is a menu of a program written for X11
      that does not say what process it is of; a program cannot yet say
      of its own window that it is private; and the panel lists every
      window by its title, a private one's too, with a title cut short
      whole in its tooltip, all of it in pictures, as the panel is not
      told which windows are private (ADR 84).
- [ ] Sharing the screen in Firefox froze once, on real hardware, just
      after a private window opened, and not again with every picture
      logged: what failed, if anything, is not known. Waits on: its
      happening again.
- [x] The stream that sharing a screen in a browser or a call needs
      (PipeWire), in buffers of the GPU so that nothing is copied, with
      the user asked each time.
- [x] What is under a pointer that does not move found again when a
      window comes, goes or moves under it: that window is told, and the
      pointer looks as it ordinarily does until it says otherwise.
- [x] The mark that says the screen is captured under a panel along the
      top, in the corner the panels leave, and not over its end.
- [x] Resting the pointer on the mark says which programs take pictures,
      of the screen or of a window on it (`tooltip` in the theme).
- [x] The program a screen is shared with named, not the portal that
      shares it: found from where its stream goes in PipeWire (ADR 43).
- [x] The mark shown while a microphone or a camera is being recorded
      from, through PipeWire, with what records named the same way.
- [x] A camera a program opens itself, not through PipeWire, seen as well:
      by which processes have the camera device open.
- [ ] Known gaps in what records: a program whose open files cannot be
      looked into, as in some sandboxes, has a camera open unseen; one
      that keeps a camera open without taking pictures is said to
      record; and with PipeWire 1.0, which does not say its own process,
      what records sound through PulseAudio is named as `pipewire`.
- [x] A pointer an application draws itself, as programs written for X11
      do, given to programs following the pointer as it is drawn, not as
      the ordinary one.
- [ ] Known gaps in pictures of the screen: a screen that is turned is
      given as it is stored, not as it is seen. There is no turning a
      screen yet; it comes with arranging the screens, below.
- [x] The portals answered by a program of our own,
      `xdg-desktop-portal-omorfos`, started with the session: so far
      the settings that tell applications light from dark and the
      accent, read from the theme and told again when it changes.
- [x] The Screenshot portal, with the user asked each time by a
      question of the backend's own, over everything, which is not in the
      picture.
- [x] The portals' question drawn as parts of the theme of its own,
      `question`, `question-detail` and `question-button`, not the
      launcher's.
- [x] An application that asks the portals named by its desktop entry,
      not by the identifier its sandbox gives it.
- [x] Colours picked for applications through the Screenshot portal,
      from a still picture of the screen, the picking being the consent,
      under a crosshair and beside a round loupe.
- [x] A screen or a region of one picked over still pictures of the
      screens, with `Shift+Print` and `omorfos-screenshot --pick` (ADR
      44).
- [x] The portal picking the same way, after the user has said yes.
- [x] Windows in the picker: the compositor saying where each is, the one
      under the pointer outlined, and a click taking it whole.
- [x] Colours picked from any screen, each shown still over its own.
- [x] Sharing the screen through the portal, as a PipeWire stream, with
      the user asked each time, and the mark on the screen while it is
      shared. The compositor puts each picture straight into memory the
      reader maps, nothing copied on the way; only while something reads
      the stream, only as the screen changes, and no more than sixty times
      a second. A reader that comes anew, as a call's preview and then the
      call, is shown the screen at once, and sharing carries on through
      the session being locked, showing nothing of it.
- [x] The screen to share picked, where there is more than one, as a
      picture is picked: the one under the pointer, by a click or Enter.
      The picker says what to do at the bottom of each screen, and a line
      round a screen that is shared or recorded says so, in nothing that
      is shared.
- [x] A layer surface with a subsurface that reaches out past it, left
      or up, drawn and pointed at where it is: it was put out of place by
      as much, which took the picker off its screen when its line of what
      to do was wider than the screen. The line is still kept within it,
      as text cut off at the edge reads better than text running off.
- [x] A screen that changes size goes on being shared at its new size:
      the stream offers it, what reads it agrees on it again, and is given
      memory made to it, of the card or shared.
- [ ] A screen changing size while it is shared has only been tried by
      the tests, which resize a screen of their own, and a shared window
      changing size by hand in Firefox. A laptop's own screen driven at
      another rate while it was shared went on being shared, by hand on
      17 November 2026; its modes are all of one size, so a change of
      size is still to be tried, with a screen that has more than one.
- [x] A window shared by itself, when the application can take one: the
      user picks it over the still screens, or the screen from beside
      every window, and it is shared whatever covers it, at its size as
      that changes. Found as it was made, and put right for screens too:
      a picture of the old size given as one of the new, and memory of
      the old size used for the new, each when the size changed just as
      a picture was asked for.
- [x] A window covered by others reached in the picker, to share or take
      a picture of: Tab outlines the next window down under the pointer,
      and round again, where before the user had to cancel, move it and
      ask again.
- [x] The window Tab reaches drawn over the rest from a picture of it
      alone, taken as the picker goes up of each window another covers,
      so that what is to be shared or taken can be seen first.
- [ ] Known gaps in reaching a covered window: its title is not said
      beside it; a window partly off its screen is shown squeezed into
      the part on it, and one whose picture holds a shadow of its own a
      little smaller; and there is no row of the windows there are, each
      with its title and picture, for one under nothing the pointer is on.
- [x] The pointer over a window that is shared, drawn into its pictures or
      sent beside them as a call asks, where it is in the window's picture:
      a program may follow it over a window as over a screen.
- [x] A line round a window while it is shared, as there is round a screen:
      over its frame and under the windows over it, and in no picture.
- [x] A region of a screen shared, drawn by a drag in the picker, cut from
      pictures of all of the screen in memory of the portal's own.
- [ ] Known gaps in sharing a region: not in memory of the card, which
      would need the GPU to cut it; a screen that changes size ends it;
      and with one screen, an application that can take only a screen is
      given it at once, with no picker to draw a region in.
- [ ] Known gaps in sharing the screen: the size the application is told
      is in pixels, not in units of layout; a choice cannot be remembered
      for next time; and the user cannot stop sharing from the desktop,
      only from the application. A reader that cannot take buffers of the
      GPU is still given pictures read back and turned on the CPU.
- [x] The pointer sent beside the pictures when a call asks, as
      PipeWire's cursor metadata, so that moving it makes no new picture of
      the screen: where it is with every buffer, its picture with it, and a
      buffer of that alone when only the pointer has changed, no more than
      sixty times a second. The compositor lets a program that may take
      pictures follow the pointer by itself.
- [x] Only what has changed copied while the screen is shared: the
      backend says, for each of the stream's buffers, what has changed
      since a picture was last put in it, and the compositor reads back
      from the GPU and writes only that, and what changes. The backend
      says in the log how the pointer was asked for, and how many pictures
      and buffers of the pointer alone it sent.
- [x] Programs hand over pictures in the memory of the graphics card
      (`linux-dmabuf`), in what the card can draw, which are drawn without
      being copied. Without, a program that draws with the card, as a
      browser does, drew everything without it: Firefox was at 95% of a
      core while a screen was shared. Tested on a card where there is one,
      and skipped in CI.
- [x] Sharing the screen in the memory of the graphics card: the stream
      offers memory of the card in each layout the compositor can draw
      into and the card can make, and shared memory last; what reads it
      takes the first it can. The compositor draws each picture straight
      into that memory, and nothing reads a pixel back or turns it. Where
      the card cannot be had, or what reads the stream cannot take its
      memory, pictures go through shared memory as before. Tested on a card
      where there is one, and skipped in CI. Tried by hand, sharing a 4K
      screen with a video playing took omorfos from 60% of a core to 18%,
      about what the screen costs it with nothing shared, without stutter.
- [ ] Known gaps in sharing in the memory of the card: the compositor
      waits for the card to finish each picture in its loop, which a fence
      the backend waits on would spare it; only layouts of one part are
      offered; and what reads the stream is not asked which layouts it
      prefers, only offered those that can be made, each in turn.
- [x] The portal's settings read from `appearance` once there is one
      (stage 4): the scheme, the accent and reduced motion. High contrast
      is told as off until there is a setting for it (stage 8).
- [x] A screen's resolution and refresh rate, as `mode` under `outputs`:
      that size at that rate, or at the fastest, and what the screen says
      suits it best if it cannot be had, which the user is told. A change
      is followed as soon as the file is saved. It is chosen in the
      configuration crate and only applied by the hardware backend, which
      no test runs: tried by hand only.
- [x] Screens above and below one another as well as side by side, as a
      listed screen's `place` says, next to the one listed before it.
- [x] A screen's place nudged along the edge it shares with the one
      before it: lined up at the start, the middle or the end of it
      (`align`), and moved along by an offset in units or as a share of
      its length (`offset`), as a laptop below a monitor and a little to
      its left is; and the user told when the first listed is given a
      place, which says nothing.
- [x] The screens and the modes each offers, in the IPC state and by
      `omorfosctl screens`, written as `mode` takes them.
- [x] A screen turned on its stand (`rotate`), drawn turned and given in
      pictures the right way up; in a window inside another desktop it is
      not used, and pictures there are the right way up too.
- [x] One screen made primary (`primary: true`): the notice is shown on
      it, the windows of a screen unplugged go to it, and it stands in for
      the first wherever there is no better screen to choose.
- [ ] Programs written for X11 told which screen is primary, as RandR
      has it, for games that go fullscreen there. Smithay asks it only by
      waiting for the X server's answer, and the X server can be waiting
      on the compositor at the time: tried, it froze the tests as the
      server started and as it was started again. Waits on: Smithay, for
      a request that does not wait.
- [x] An arrangement of the screens for each set that may be plugged in
      (`arrangements`), followed as screens come and go.
- [ ] A screen once plugged in remembered, and a scale that suits it
      chosen for it (asked on 18 November 2026). Today a screen is told
      from another by the socket it is on (`name: HDMI-A-1`), so that
      another monitor on the same socket has the last one's settings;
      and nothing chosen while running is kept. `output.scale: auto`
      took how dense a screen is, over 120 to the inch, whatever kind
      of screen it is, until ADR 100. On the maintainer's laptop, 3840
      by 2160 and some 280 to the inch, that chose 2.5, which is too large: 1.75
      is what suits, and 2 will do, which is some 150 to the inch; and
      a scale of 2 set for every screen, to put that right, is far too
      large on the monitor then plugged in. A screen read from nearer
      can have things smaller on it. To do, in this order:
  - [x] What the market chooses, found out before a rule is settled:
        the scale other systems give by default to the panels there
        are, laptops by size and resolution, monitors on a desk,
        televisions; written up, with the laptop above as one point:
        done on 19 November 2026, by Fable 5.2, from the other
        systems' code and documents and with nothing run, and written
        up in
        [reviews/2026-11-19-what-scale-other-systems-choose.md](reviews/2026-11-19-what-scale-other-systems-choose.md).
        The market gives that laptop 2 to 2.5, and 1.75 is nobody's
        default; and every system that aims at a density aims a laptop
        denser than a monitor, where omorfos has 120 for both. It says
        what that leaves to decide for the guess.
  - [ ] What the survey could not find out, found on the machines
        themselves: what the laptop's panel says its size is, which
        for the old rule's 2.5 is 342 mm wide or less where 15.6 inches is
        345; and what Windows and macOS choose where the survey has
        only users' word for it, over 250% and on a television. The
        panel, read from its EDID on 23 November 2026, is a BOE
        NV140QUM-N53 of 14 inches, 309 by 174 mm, 316 to the inch, for
        which ADR 100's rule gave 2.5: 2 is now taken up to 0.35 over
        it, which gives it 2 and changes no screen of the survey's. It
        names no model in its name and no serial, which a screen known
        by what it is has to allow for. What Windows and macOS choose is
        still to be found.
  - [x] A screen known by what it is: its maker, model and serial, as
        it says them, and the socket only where it says none. What is
        set for a screen by its socket's name goes on working. Done
        through names of the user's in `screens` (ADR 103), which
        `outputs`, `arrangements` and `mirror` take in place of a
        socket's; read from the EDID's first block by `omorfos-core`,
        and shown by `omorfosctl screens`. Not yet seen on hardware, as
        the backend that reads the EDID is run by no test. Known gap:
        `wallpaper.screens` in `config.yaml` names screens by their
        sockets still.
  - [x] A better guess: from how dense a screen is and how far it is
        read from, by the kind it is (a laptop's own, a monitor, a
        television, told by its size), rounded to a
        whole scale where one is near, those being the sharpest. A
        screen that says nothing of its size, or what cannot be so, as
        televisions and projectors do, is guessed from its resolution
        alone, and never so small or so large that it cannot be used.
        The rule is settled, in
        [ADR 100](adr/0100-a-screens-scale-by-how-near-it-is-read.md),
        from what the survey found: 135 to the inch under 20 inches
        and 110 from there, told by size and not by socket; quarters
        up to 2 and halves beyond; a television, and a screen that
        says nothing true, by resolution alone. It gives the laptop
        above 2. Built in `Scale::for_screen`, with what it gives each of
        the survey's screens as its tests, and guessed again when a
        screen is driven in another mode; not yet seen on a screen, as
        the hardware backend that asks it is run by no test. Known gap:
        when this rule is first taken up by a change of compositor
        under the relay, a screen beside one whose scale changed moves,
        and windows come back at their places in the old layout, on
        the next screen or off every screen, as they would with any
        change of layout between compositors
        (finding 2 of the review of pull requests 44 to 48):
        a kept window not on its screen is to be put back on it.
  - [x] Everything of a screen remembered once it has been plugged in:
        its mode and how often it is drawn, its scale, where it stands
        among the others, which way up it is, and whether it shows what
        another shows; and plugged in again, at any socket, it is as it
        was. In three layers, each over the one before, as the rest of
        what is settled is to be: what omorfos guesses; what the
        session has kept for that screen; and the user's own files. A
        guess is made once, when a screen is first seen, and kept: one
        made later never takes the place of what is kept, through a
        screen pulled out and plugged in, sleep, and the compositor
        started again. Done for the scale guessed at each size, and the way the display
        key keeps, in `$XDG_STATE_HOME/omorfos/screens.yaml` (ADR 104);
        the mode is what the screen says each time. Left for Settings:
        its place and rotation chosen while running.
  - [x] Guessing again when asked: `omorfosctl`, and Settings when
        there is one, has omorfos guess anew for a screen and keep
        that in place of what it kept; the user's own files are changed
        only where the user says so. Done as the `guess_scales` and
        `guess_scale` actions, which `omorfosctl action` and keys
        reach; Settings, when there is one, is to offer them.
  - [x] Known gap to close with it: `omorfosctl state` says how large a
        screen is in units of layout, after its scale, which reads as
        its resolution and is not: both are to be said. The state and
        `omorfosctl screens` say its scale and its size in pixels beside
        its size in units.
- [x] The windows of a screen that was unplugged put back on it when it
      is plugged in again (asked on 17 November 2026): today they go to
      the primary screen and stay there. Each window is to go back to
      the screen and the workspace it was on, at the size it had, unless
      the user has moved or resized it since, which is taken as its
      being where they want it. A screen is known again by its own name
      (EDID) where it has one, and by its socket otherwise. Done in the
      desktop model (`remove_output_known`, `bring_back`), by what the
      screen says it is, with its socket where it says no serial, as its
      kept scale is; a screen turned off and on again brings its windows
      back too. Known gap: what is
      kept lasts while the compositor does, and is lost if it is started
      again while the screen is unplugged.

What is left of this stage that is new, in the order it is to be done,
as decided on 2026-10-21: what is felt every day first, the lock screen
late so that it is written and reviewed with the care it needs, and
what only games and touchpads need last.

- [x] Keys for volume, brightness and media, with a small display of the
      level over the screen that goes by itself: `omorfos-session`, run
      once a key, sets the volume by `wpctl`, the brightness by logind,
      and tells players over D-Bus, and the level shows as a
      notification with a bar that the next press changes (ADR 46).
- [ ] Known gaps in those keys: a key held down changes the level once,
      as bindings do not repeat; none of them works while the screen is
      locked, which only our own lock screen should change; the
      keyboard's backlight has no keys; and the brightness of a screen
      outside the laptop, which only speaks DDC, is not set.
- [ ] More of a keyboard's own keys. Before any is bound, what each
      sends is checked on a real keyboard (`wev`), since makers differ.
      The touchpad switched off and on by its key (`XF86TouchpadToggle`),
      as an action of the compositor's, which has the touchpad already.
      The key marked with a star (`XF86Favorites`), which is made to be
      the user's, left unbound and written down as one to bind; what it
      does by default, a browser perhaps, is for the review of the
      defaults. Search and the calculator (`XF86Search`,
      `XF86Calculator`) open the launcher, the second once it does sums.
- [x] A switcher for `Alt+Tab` (wish 5): the windows in the order they
      were last used, each with its picture and title, walked through
      while Alt is held and chosen when it is let go. Flat first. The
      windows standing one behind another on a diagonal, turned through
      as a queue, is the same thing drawn another way and comes with
      stage 4.
      The order is that of use, most recent first, and is remembered
      between one use of the switcher and the next. While Alt is held,
      Tab goes through all of the windows and the order stands still:
      a window passed over on the way has not been used. When Alt is
      let go, the window chosen goes to the front of the order and the
      rest keep their places, so that one Alt+Tab goes back to the
      window before and another goes back again.
      Drawn by the compositor, each window small in its place over its
      title, as ADR 38 has it.
- [x] The switcher's icons: each window with the icon its program gives
      it (`xdg-toplevel-icon`), or its application's otherwise, and a
      private window shown by its icon as well as its title.
- [ ] The icons of windows elsewhere: the panel's list of windows, and a
      program written for X11, which gives its icon as `_NET_WM_ICON`.
      The icon theme is `hicolor` and then Adwaita, as for the shell's
      components; the user's own theme comes with the look.
- [ ] Known gaps in the switcher: windows are made small in one step,
      not by halves, so text in them is rough, and rougher still at the
      back of the queue; and the pointer moving over a window does not
      pick it.
- [x] Sleep, restart and shut down, by logind, and what the lid and the
      power button do. Sleep, by the lid or asked for, locks the screen
      first, with the locker in use until `omorfos-lock` takes its place,
      and waits for it to be locked, so that the machine never wakes
      unlocked. `omorfos-session` runs all session to do it, and
      `loginctl lock-session` locks too. The lid and the keys for sleep
      and for turning off are logind's, which already acts on them, so
      they are not bound (ADR 35).
- [x] Locking and sleeping when the user is away, after the times the
      configuration gives (`idle.lock`, `idle.sleep`), with no idle daemon:
      each once a time away, and held off by what is being watched.
- [x] Staying awake as the user asks (`stay_awake`, `stay_awake_for`, and
      in the root menu): nothing locks, goes dark or sleeps until it is
      asked again, its time is up or the session is locked by hand; the
      panel says so, until when, and a click there lets it go.
- [x] Programs that ask to be kept awake other than by Wayland heard too:
      by `org.freedesktop.ScreenSaver`, as Chromium, Electron, VLC and
      programs written for X11 ask, and by the portal's `Inhibit`, as
      programs in Flatpak do; each held only while the program that asked
      is there, and keeping the machine from sleeping also while the
      session is locked, so that music behind the lock screen plays on.
- [x] Keys and passphrases forgotten when the session is locked, as a
      choice (`lock.forget`): what `ssh-agent` holds (`ssh-add -D`), what
      `gpg-agent` has kept, and the PIN a smartcard was given, its daemon
      ended, so that a locked machine left behind has no key to use. Off
      unless chosen, since every key is to be given again after (ADR 14).
- [x] The radios, wireless, Bluetooth and the rest, known to omorfos
      whatever turned them off or on: a key the kernel handles itself (on
      a ThinkPad, `thinkpad_acpi`), a switch on the side, or another
      program. Each change is shown as the volume is, "Bluetooth off",
      with an icon for it; and the panel shows which are off, aeroplane
      mode when all are, from the compositor's IPC state, which gains the
      radios and says when they change. The keys are not bound: the
      kernel has already acted on them, and a binding as well would undo
      it. The radios are read from `/dev/rfkill`, which tells of each
      change as it happens, by `omorfos-session watch`, whose logind half
      is now its optional part, and which tells the compositor. The
      panel says "Aeroplane mode", or which are off.
- [x] Writing in languages that need an input method
      (`text-input-v3`, `input-method-v2`, `virtual-keyboard-v1`), for a
      program given the `input` role, its popup under the text and no key
      shown it while locked (ADR 47); and switching keyboard layout by
      the `next_layout` action, which no key is bound to.
- [ ] The layout shown: which one is being typed with, on the panel, and
      a key for `next_layout` decided with the other default keys.
- [x] A second input method refused, not the first: Smithay told the one
      there already that it may no longer be one and left the second
      untold, so the compositor answers for the input method itself and
      hands the first to Smithay. The fix is to be offered to Smithay, and
      this let go of once it has it.
- [x] Programs listed in `~/.config/autostart` started with the session,
      and the session's environment told to D-Bus and systemd, without
      which portals and much else do not find it. The environment was
      told already; the entries are started once at login by
      `omorfos-session autostart`, the system's too, as the specification
      has it.
- [x] A polkit agent, so that a program can ask for the administrator's
      password (wish 9): `omorfos-ask`, in the sealed prompt, the session
      dimmed behind it and the keyboard and pointer its alone, what is
      typed hidden unless asked to be shown (ADR 48).
- [x] `omorfos-pinentry` for keys and passphrases, GnuPG's and the SSH
      keys `gpg-agent` holds, in the same sealed prompt, in words of
      omorfos's alone, the key named by what GnuPG lists of it (ADR 58).
- [x] What `gpg-agent` asks to be confirmed, as a key added with `ssh-add
      -c` is each time it is used: decided to be said no to still, and
      `SSH_ASKPASS` not offered, as neither says what is asked, or of
      which key, but in a program's words (ADR 58).
- [x] The session blurred as well as dimmed behind the sealed prompt,
      once Glass has brought the blur: `sealed`'s `backdrop-filter`
      (stage 4).
- [ ] Screen readers: what the shell's components draw, the sealed prompt
      first, told to them over AT-SPI, as GTK and Qt programs are. Our
      components draw themselves, so each has to say what is on it and
      what it does; AccessKit, which speaks AT-SPI over D-Bus with zbus,
      is the likely way. Until then the eye's tooltip says what it does to
      those who can see it, and nothing tells one who cannot.
- [x] Wallpaper (wish 6, ADR 49): `omorfos-wallpaper`, a colour or a
      picture per screen, filled, fitted, stretched, centred or tiled; a
      directory shown in turn, by name or shuffled, at an interval. PNG
      and JPEG, a photograph turned as the camera was held, a large PNG
      read a row at a time and no larger than the screen needs. What each
      screen was given is kept, and shown at once at the next login; a
      directory starts where it was kept, and its next picture is read
      before it is due. With nothing named it puts nothing up, and the
      theme's `desktop` shows.
- [x] Wallpaper, a fade from one picture to the next, done by the
      compositor's renderer through `wp_alpha_modifier_v1` (ADR 49).
- [x] The brand (ADR 55): the mark, the name, the icon and the
      wallpapers, drawn by `omorfos-brand` as omorfos is built and
      installed with it; the design language's grounds and accent taken
      from it. With nothing named, the wallpaper shows the one that came
      with omorfos, and a picture may be a pair, one for a dark desktop
      and one for a light one, shown as the theme is.
- [x] A directory of pictures that keeps pairs by their names (`-dark`
      and `-light`), so that a directory shown in turn follows the theme
      too.
- [x] More wallpapers that come with omorfos, drawn by `omorfos-brand`
      beside Prism, each a dark one and a light one: Spill, Horizon,
      Facets and Ground.
- [x] A choice of the wallpapers offered the first time the desktop is
      set up, with the rest of what is chosen then: on the welcome card.
- [ ] The lock screen's picture of the brand, drawn by `omorfos-brand`
      as the wallpapers are, its mark small in the bottom right, once the
      lock screen has a picture of its own (below).
- [x] Wallpaper, a directory watched, so that a picture put there or
      changed shows at once and not at the next turn; and a picture named
      read again when it changes.
- [ ] Wallpaper, what is left: other formats (WebP, AVIF, SVG) as they
      are asked for, each another decoder of untrusted input.
- [x] A screen that mirrors another (ADR 15): `mirror` in `outputs` and
      `arrangements`, the other's picture as large as fits, out of the
      space and unknown to programs, and hidden when the session locks.
- [x] Key bindings made beside the built-in ones (ADR 50): one written
      on keys a built-in binding uses takes its place, `unbind` does
      without one, and `defaults: false` has only what is written.
- [x] A screen turned off (ADR 15): `enabled: false` in `outputs` and
      `arrangements`, out of the space and unknown to programs, turned off
      by the backend; never every screen plugged in.
- [x] The display key (`XF86Display`, ADR 51) goes through the screens
      as configured, mirrored, and each alone, each kept only if Enter is
      pressed within 15 seconds and put back otherwise, since it is the
      key pressed when a screen has gone dark.
- [ ] A change to the screens made from Settings (stage 7) kept only if
      the user says so in time, and put back by the compositor itself
      otherwise, so that a screen left dark comes back on its own.
- [x] A lock screen of our own, `omorfos-lock` (ADR 52): the time and the
      password on every screen, PAM asked through `nonstick` by a helper
      for each way, the password, a finger and a security key at once,
      whichever is first unlocking; the locker started again if it ends
      with the session locked. Chosen with `locker: [omorfos-lock]`.
- [x] Only the time on the lock screen until a key or a button is
      pressed, and then the user's face, or their initials, and full name,
      the field and the ways rising into view; the time alone again after
      30 seconds left alone; a way other than the password shown only once
      it speaks, so that a key is shown only while it is plugged in
      (ADR 52).
- [x] Sleep, Restart, Shut Down and Log Out from the lock screen, without
      unlocking (ADR 53): asked of `omorfos-session`, each offered only
      where logind lets it be done without a password; all but sleep
      asked about first; who keeps the machine on said; `lock.power` to
      have none of them.
- [ ] The lock screen's Sleep, Restart, Shut Down and Log Out each with the
      sign the root menu gives it (a moon, a door, an arrow turning back,
      a switch's), drawn through `omorfos-icons`, as the maintainer asked
      on 15 November 2026.
- [x] A quieter log, begun (as found in the log of a session on
      2026-10-23): a program disconnected for breaking the protocol is
      said, by name and process, and why; the components end quietly when
      the session does, telling that from being sent away; what the
      Wayland library would print of it, unsigned, it no longer does; and a
      setting named as the guide names it, `lock.power`, is said how to
      write.
- [x] Visual Studio Code, which was disconnected seconds after it
      started for destroying its icon's buffer before the icon, made to
      work: window icons served by omorfos, an icon's lost pixels let go
      (ADR 54).
- [x] The programs the session started asked to close before it ends, and
      given two seconds, so that they end as on any desktop and not with
      the compositor gone from under them; asked again, it ends at once.
- [x] What the programs the session starts write kept in the journal, each
      under its name, and not in omorfos's log: those the compositor
      starts, and those the autostart entries list.
- [x] New versions taken up without logging in again (ADR 59). The
      programs that come with omorfos started again when their files are
      installed: the clipboard keeper handing what it holds to the new
      one first, the portal waiting for a screen being shared to stop,
      the agent for its prompt to go. The compositor itself cannot be
      swapped under its windows; its new version is said, to be used at
      the next login.
- [ ] Buffers in shared memory released as soon as they are drawn,
      which foot asks for ("compositor is not releasing buffers
      immediately"). Waits on: Smithay, where it is to be done, not here. Smithay keeps a
      buffer until the next is attached, and has no way to let it go
      sooner; niri, on Smithay too, does the same. Sending the release
      from omorfos would release it twice, and is right only where the
      pixels were copied: the card's renderer copies them, but the
      software renderer of the tests reads them in place, and a second
      card copies them only when it draws, perhaps after the program has
      drawn into them again. What it costs is small: foot copies what
      changed from one buffer into the other, and since 1.25.0 off its
      main thread. foot 1.21.0 also says window icons are not offered,
      which they are: it looks for the wrong name, fixed in foot since.
- [x] The lock screen's buttons reached with Tab and Shift+Tab and
      pressed with Enter, the one the keyboard is on drawn
      `question-button:focus`, and Cancel the one it is on when asked
      whether to go ahead. Escape, unless it says no to that, shows only
      the time again, as being left alone does; either way the rest goes
      as it came, and turns back from where it is if woken meanwhile.
- [ ] What logind's inhibitors say shown whole on the lock screen, which
      ADR 53 weighs.
- [x] The administrator's prompt's buttons, the eye, Cancel and
      Authenticate, reached with Tab and Shift+Tab and pressed with Enter,
      as the lock screen's are.
- [x] The fields a secret is typed into, on the lock screen and in the
      administrator's prompt, behaving as other fields do: Backspace held
      down taking out one character after another, as keys repeat
      (`wl_keyboard.repeat_info`); Ctrl+A choosing all that is typed, to
      take it out or type over it at once, with Super+A as on a Mac,
      marked `question-input:active`; and a text cursor in the lock
      screen's field, where the prompt has one. Only what types and
      Backspace repeat, so that Escape or Enter held does one thing once.
- [x] Ctrl+A in a layout that is not Latin, as Greek or Russian, where
      the key types another letter: the fields take the Latin letter the
      key has in the first of the layouts where it has one, as programs
      do. With no Latin layout beside it there is none to take.
- [ ] `omorfos-lock` the default locker, once it has been used for a
      while; its own parts in the theme; a field that shows what it asks
      as PAM says it; the on-screen keyboard (stage 7) usable on it; and
      what is typed cleared from memory with `zeroize`, weighed as ADR 52
      says.
- [x] A dialog of one program belonging to a window of another
      (`xdg-foreign`), as a portal's file chooser does, which opens where
      that window is, over it if it is fullscreen.
- [x] Programs told when what they drew reached the screen
      (`presentation-time`), by the card's own clock and count of
      refreshes, for a video to keep time with its sound. A nested
      compositor's host says nothing of it, so there it is the time it is
      heard of.
- [ ] Smaller protocols that programs complain of lacking, each waiting
      on something different, gone through on 20 November 2026 against
      what the others offer ([the review](reviews/2026-11-20-the-protocols-programs-lack.md)),
      with an order: `xdg-dialog`, `xdg-system-bell` and
      `xdg-toplevel-tag` first, then `xwayland-keyboard-grab`, then
      `fifo` and `commit-timing` once a held commit can be let go, then
      `linux-drm-syncobj` and `tablet-v2`, which need the relay and real
      hardware. A pen gets nothing today, not even a pointer. `keyboard-shortcuts-inhibit` is offered, by
      ADR 78's rule. Content type, which Smithay has: it says what a window
      shows, for a screen to be driven to suit it or its refresh to
      follow a game's, and waits on omorfos doing one of those, rather
      than being offered to quiet a program's log. Tearing control waits
      on Smithay, which does not have it yet.
- [x] What games and remote desktops need of the pointer: locked,
      confined, and moved without a position (`pointer-constraints`,
      `relative-pointer`), held only while the window has the keyboard
      (ADR 57).
- [x] Gestures on the touchpad: workspaces by a swipe of three fingers,
      and every other gesture the program's under the pointer
      (`pointer-gestures`).
- [x] The swipe following the fingers: the workspaces slide with them
      and settle over 320 ms once they lift, onto the next if they went
      past half way or were flicked, and back otherwise. Half way is of a
      fixed way of the fingers, not of the screen, which asked a long
      swipe and a fast one at once on a wide screen; either is now
      enough, but for a twitch.
- [x] The same slide when the keys go between workspaces: the one they
      bring in from the side it lies on, with the one they left beside it
      however far apart the two are, and a key pressed again taking them
      on from where they are. What slides in or off with its workspace
      does not fade.
- [x] The row of workspaces not going round: nothing before the first,
      and after the last only a new one, empty, made when the last has a
      window on it.
- [x] Two workspaces sliding side by side told apart: the wallpaper goes
      with them, drawn for each, where the windows of one ran on into the
      other's over a wallpaper that stayed still.
- [x] A workspace sliding kept to its own screen (seen by the maintainer
      on 22 November 2026): with a laptop's screen and another beside
      it, going to the next workspace or the one before showed the
      windows sliding in, or out, on the other screen too, as they
      crossed the edge between the two. Every screen draws from the one
      space all share, and a sliding workspace's windows are moved
      across in it. While its workspace slides, a frame now says the
      area of its own output (`Frame::within`), and a window is drawn
      on that output alone; one that is still is drawn wherever it
      reaches, as one across two screens is.
- [x] A window flashing on the screen beside, as three fingers carried
      a workspace the whole way across and lifted (seen by the
      maintainer on 23 November 2026, and found with a build that logged
      each frame): the slide then ends at once, its windows were taken
      for windows that had gone, and a copy of each fading where it was
      last drawn, a whole screen across, was drawn on the screen beside.
      A window that had slid wholly off its screen has slid away, and
      leaves no copy.
- [ ] Known gaps in a workspace sliding beside another screen, found
      with the fix and not looked at further: a window moved across into
      the screen beside is still there in the space, so a click on that
      screen while the slide lasts may reach it, and its program may be
      told it has entered that screen, at that screen's scale.
- [x] Nothing sliding with `appearance.reduce_motion`: the keys change
      the workspace at once, and three fingers carry nothing, changing it
      as they lift if they went far enough or fast enough (ADR 71).
- [ ] Four fingers as well as three, decided with ADR 101 on 24 November
      2026: three or four, as a Mac, with down for the program's
      windows, a pinch for every program and a spread for the desktop,
      and a page of settings that shows each gesture and changes it;
      built with the keys of stage 4.
      - [x] Four fingers swiped as three (#160).
      - [x] Four fingers spread carry every window of the workspace
            shown straight out from the middle of the screen, a sliver of
            each left at an edge, and pinched carry them back
            (`show_desktop`, #158), as do Escape, a sliver clicked, a
            window chosen or opening, and leaving the workspace, as it
            slides away; nothing of a workspace is drawn over the one
            sliding in beside it. The
            workspaces, the overview and the windows aside follow the
            fingers by one rule, `fingers.rs`.
      - [x] Down, with nothing open, the program in front's windows
            alone, and up closing them (`show_program_windows`).
      - [ ] Four fingers pinched, the launcher's every program (#157).
      - [ ] The page of settings.
- [x] The session outliving the compositor (ADR 60), the holder first,
      `omorfos-hold`: a small program that binds the socket and starts
      the compositor with it, and again when it ends, with a new version
      installed meanwhile, or the old one it holds open when the new does
      not come up; a locked session coming back locked, the X display
      keeping its number, and what starts with the session started once
      a login; `omorfos` on the hardware becoming the holder when there
      is none above it; the holder's own new version taken up in its
      place, checked first; and an action asking the compositor to step
      down, for trying a build on real hardware without logging out.
      Programs close with the compositor, Qt 6's apart, until the relay
      (below), so nothing asks for a swap but the user, and ADR 59's
      word about the next login stands.
- [ ] Known gaps in the holder: on real hardware it has been tried by
      hand, started from a console and by a login screen, GDM, but
      the backend's part of it, taking the connection and becoming the
      holder, is run by no test; a compositor that hangs without ending is
      waited for for ever; and `restart_compositor` has neither a key
      nor an entry in a menu. The keyboard layout chosen by `next_layout`
      is now told to the next compositor, which types with it.
- [ ] The compositor ending when its virtual terminal is left: on real
      hardware, switched to another, it drew to screens it no longer held
      ("cannot draw on eDP-1 … Permission denied") and then ended as
      failed, saying nothing more, so the holder started it again; it is
      to wait until the terminal is shown again. The holder now says how
      a compositor that failed ended, the signal or the exit status, so
      the next time it happens the log says whether it crashed or gave up.
      On 11 November it outlived a switch to another terminal and back,
      saying only that it could not draw meanwhile.
- [x] Every protocol the compositor offers listed in code, each with how
      a program's state in it would be replayed, or that there is none,
      and a test that what is offered is the list; from then on a
      protocol is offered by being added to it with its note, for the
      relay to be built from.
- [ ] Known gaps in the list of protocols: its notes were written before
      the relay, from what each protocol says, and are a first word that
      building the relay will correct; nothing reads the list yet but
      its tests.
- [x] The relay, `omorfos-relay` (ADR 60), in the path (ADR 61, step 1):
      started and kept by the holder, handed the socket and joined to
      each compositor by a connection only the holder makes; reading
      every message both ways against the list of protocols, with a
      table of each program's objects, and dropping a program that
      breaks a bound; telling the compositor who each program is, which
      it takes in place of asking the socket; fuzzed by `cargo-fuzz` in
      a nightly CI job of its own. A restart still closes every window.
- [ ] Known gaps in the relay: it makes its list of what to wait on anew
      each turn, where epoll would keep one; and one that hangs without
      ending is not noticed. One that ends is: the compositor sees its
      connection to the relay close and steps down at once, and the holder
      starts a relay anew with the next.
- [x] Programs kept by the relay, the means (ADR 61, step 2a): a
      program's connection outliving the compositor; what it has made
      written down as it is made, and made again for the next compositor
      under the same numbers; what that one says sifted until it has
      caught up; the pointer and the keyboard leaving and entering again;
      the relay handed over when the holder takes up its own new version.
      For the protocols the tests' client speaks, and tested with two
      compositors one after the other.
- [x] Programs kept by the relay, `foot` (ADR 61, step 2b): what `foot`
      makes of the protocols the compositor offers made again, so that a
      terminal and what runs in it outlive a restart: decorations,
      viewports, single-pixel buffers, fractional scales, the shape of
      the pointer, screens' sizes (`zxdg_output_v1`), fields typed into,
      told what is typed by their own count of commits, and a surface's
      parts, stacked and committed as they were. What says how something
      is is not told again as it was. A program let go is named, with
      everything it held that cannot be made again.
- [x] Programs kept by the relay, GTK's and the browsers' (ADR 61, step
      3a): buffers on the graphics card made again from the descriptors
      held, touchpad gestures, idle inhibitors, window icons and exported
      windows; popups, tokens to hand focus on and word of a picture shown
      ended as the compositor goes, as it would have ended them, and the
      relay answering for each after; a gesture under way told it was
      cancelled.
- [x] Serials kept to their meaning through a change of compositor (ADR
      61, step 3b): the next compositor's told past every serial the
      program has, and taken back as they are asked with, one before's
      asked with as the next's that stands for it; the pointer and the
      keyboard, back where they were, not said to have left, but where a
      menu ends with the compositor; what the next first says of a window
      answered by the relay alone; a buffer the compositor held released
      as it goes, unless the next is shown it.
- [x] What was on the clipboard there again after a change of compositor
      (ADR 61, step 3b): the keeper kept as programs are, with its role,
      and not started again; its selection set again at once through
      `ext-data-control-v1`; what a program selected set again as it next
      has the keyboard, unless something else is selected by then.
- [x] A window back where and as it was after a change of compositor (ADR
      61, step 3c, the first part): each window's output, desktop, place,
      size and states, stacking and focus, and which desktop each output
      shows, which has the keyboard and where the pointer is, kept by the
      relay as the compositor tells it and handed back to the next; a
      window's icon given it again.
- [x] Tiles and tab groups as they were after a change of compositor
      (ADR 61, step 3c, the second part): a window that shared a frame
      joins it again among its tabs in their order, the one shown shown;
      one that shared a workspace has its part again, the workspace cut
      as it was and shown if it was.
- [x] Programs in a sandbox kept through a change of compositor (ADR
      61, step 3d, the first part): the socket a sandbox hands over is
      the relay's to listen on, and each program that connects there is
      kept as any other, and handed to each compositor as one inside a
      sandbox.
- [x] Layer surfaces made again after a change of compositor, and the
      programs the configuration starts with a role kept through it, so
      that a bar such as waybar stays (ADR 61, step 3d, the second part):
      each layer surface as it was, told its size again only where it
      changed; each such program handed to the relay with its roles,
      marked as the user's, so that none is taken for the keeper.
- [x] The components that come with omorfos kept through a change of
      compositor (ADR 61, step 3d, the third part): the panel, the
      wallpaper, the notifications and the portal kept as the keeper is,
      each known again by the next compositor by the process the desk
      record names, not started again, and watched over as if it had
      started it; each made to speak to the script socket again once the
      compositor is back, and the portal to ask again what it holds.
- [ ] Known gaps in keeping programs, after step 3d: a program holding a buffer
      on the graphics card the compositor made, by `create`, is let go, and
      so is one with a surface on a screen the next compositor does not
      have, where a layer surface on it would rather be closed, or one holding
      anything else no protocol above makes again; a handle exported before and passed on is the
      old compositor's to a program importing by it only after, and a window imported again
      belongs to no window for a moment first; what the next compositor
      says of a screen while the relay catches up is kept from the
      program even where it changed; what a program selected is selected
      again only once it next has the keyboard; a component kept from the
      compositor before that stops is taken to have failed, as how it
      ended cannot be known, and counted against how often it may; the
      prompt for a secret ends with the compositor, and is started again;
      the keeper is known by its role where the desk record names no
      parts, which is to go once no relay still holds one handed before
      they were named; the part of a window that did not come back, as one
      of a program written for X11, is left with nothing in it, and a
      workspace made for windows to share is shown only once the first
      of them is back; a field leaves and enters again; a part
      placed is taken to be stacked at once, where the compositor stacks
      it as its surface is next committed; touch is not made again, as
      the compositor offers none yet; a virtual keyboard's modifiers are
      not said again, as its keymap has to come first; a window in a list keeps the identifier the first
      compositor gave it, where one listing windows anew, and IPC, are given the next's; a window
      kept that the next compositor never lists leaves its handle open, and each window listed
      after it held back for a moment; what an input method was composing as the compositor
      goes is lost, as when the field loses the keyboard; pictures of a window stop as the
      compositor goes, as when the window closes, and a call sharing one stops sharing it, where
      pictures of a screen go on; a program a key starts with the role to lock that holds the
      lock is made again, but the next compositor, not knowing it for the locker, may start its
      own beside it, the second to lock being refused; a program sharing the screen that holds
      the X server for it, as Firefox does, closes as the share ends, its X server having gone with the compositor
      before; a program on Wayland that holds a connection to the X server all the same, as
      Firefox ESR does wherever `DISPLAY` is set, as it always is in a session, closes as the
      compositor goes, Xlib ending it as the X server goes with it (#124), which the check of
      real programs does not show, as it starts each with no `DISPLAY`
      (to do, decided on 24 November 2026: that check given the X server,
      for the loss to show; and keeping the X server through a change of
      compositor the first item under "Programs written for X11"); a global the next compositor offers
      once the relay's registries are answered, while it makes the rest
      again, is not told to the program, and is taken for one of the
      compositor after's; and on real hardware only `foot`, ghostty and
      Firefox ESR have been seen kept.
- [x] The pointer held made again after a change of compositor (ADR 61,
      step 3e, the first part): each lock and confinement made again,
      one held let go of as the compositor goes and held again by the
      next, one held once only and held already over; and each relative
      pointer made again.
- [x] What tells a program the user is idle, and the keyboards programs
      type with, made again (3e, the second part): each notification of
      the user being away made again, one that had said so told the user
      is back as the compositor goes and away again once the next has
      counted its timeout; and each virtual keyboard made again with its
      keymap.
- [x] The list of windows made again (3e, the third part, first half):
      each window in a program's list (`ext-foreign-toplevel-list`)
      that is kept matched to the one the next compositor lists, by the
      window the compositor hands the relay each identifier is, and
      told only what changed of it; one not kept told it has closed.
- [x] A window imported made again (3e, the third part, second half,
      `xdg-foreign`): one imported by the handle of a window kept is
      imported again by the handle the next compositor shares it by,
      once the program sharing it is made again, with the window made to
      belong to it; one whose window was not kept told it is gone.
- [x] Input methods made again (3e, the fourth part), with their
      keyboard grab and popups, as fcitx5 and IBus make them: one typing
      in a field told it is not as the compositor goes, with a key held in
      its grab let go of, and typing in it again once the field is enabled
      again; what it commits counted as the next compositor counts; one
      told another is the input method not made again.
- [x] Capturing the screen made again (3e, the fifth part): each source
      of a screen, session and follower of the pointer made again, the
      follower told the pointer left as the compositor goes; a picture
      asked for told it failed, and one not yet asked for made again with
      its buffer; what a buffer has to be told again only if the next
      compositor says otherwise. What takes pictures of a window is told
      it stopped, as when the window closes. The portal asks again for a
      picture of the pointer that failed, which it did not, so that the
      pointer beside a screen shared stopped changing its look after the
      session was locked.
- [x] Pictures of a window made again after a change of compositor:
      the source, and what was made of it, held back as the compositor
      goes; once the next lists the window, the relay making, on the
      program's connection, a list of windows of its own while the program
      waits, finding the window by the identifier it is listed by, making
      the source of it, what was made of it and each picture asked for
      meanwhile, and letting the list go. Stopped where the window is not
      kept, its program goes or it closes; one hidden while it is awaited
      waits until it is shown again.
- [x] The lock made again (3e, the sixth part), with the holder's word
      on whether the session is locked: the locker kept by the relay as
      the components are, rather than one started anew, its lock and lock
      surfaces made again, and it not told again that the session is
      locked; the next compositor starts a locker only if none came back,
      or the one kept has not locked the session again within five
      seconds.
- [x] The compositor's own new version taken up at a quiet moment (ADR
      61, step 3f), as ADR 59 takes up the others', and its word about
      the next login said only where nothing keeps the programs: once the
      relay has handed over all it kept, with nothing fullscreen, nothing
      recording, playing or keeping the session awake, the sealed prompt
      and the launcher closed, and no input for five seconds; after an
      hour regardless (`update.wait`), the next compositor saying so; and
      never while a program written for X11 has a window, as it would
      close with its X server, which the user is told, nor while the
      screens are off, which the next would turn on.
- [ ] Known gaps in taking up the compositor's own new version: a program
      written for X11 with no window open, as one in the tray, still
      closes as it is taken up, until Smithay can take up a running X
      server (stage 8); so does any program the relay lets go (known gaps
      after step 3d), unasked. Tried on real hardware on 17 November
      2026: taken up ten seconds after it was installed. `wl_touch`
      is made again once the compositor offers touch.
- [ ] A new version of the compositor that waits said to, and what for:
      in `omorfosctl state` and in the report, that one is installed and
      not yet taken up, since when, and what holds it back, a window
      fullscreen, something recording or playing, the session kept awake,
      the launcher or the sealed prompt open, a program written for X11,
      the screens off, or input too lately. Nothing says so today, in the
      state or the log: on 17 November one installed fifty seconds into a
      login was not taken up in the eight minutes until the login ended,
      and why could not be found; installed again later, it was taken up
      ten seconds on.
- [x] The compositor's own new version checked before it is taken up:
      run, asked only for its version, and not taken up where it cannot be
      run or fails, which the user is told, rather than replaced by the
      version held as after a crash.
- [x] Sound playing holds the compositor's own new version back, as a
      film kept awake does, where the program playing asks for neither:
      the watcher of what records says what plays, from PipeWire, and it
      is on no mark.
- [x] A change of compositor unseen (ADR 62), the first step: nothing
      shown until what the relay kept is back, so that the wallpaper, the
      panel and the windows are in the first frame, rather than coming back
      one by one; the relay says `ready` once each program it kept has
      caught up, and a compositor not told within a second shows the
      session regardless.
- [x] A change of compositor unseen, the second step: the picture before
      kept on the screens through the change. The compositor stepping down
      to be started again hands the holder its card, open, and gives it up
      without letting go of what it drew, which would put the console back
      and take the picture off the screens; the holder hands the card to the
      next, which takes the screens as they are, each on what drives it
      already, sets a mode only where it is not the one a screen is in, and
      lets the card go once each shows what it draws. Tried on a laptop's
      own screen: no mode is set anew, but the console shows for about a
      fifth of a second, as logind, with no one in control of the session
      for that moment, puts the virtual terminal back into text (ADR 62).
      With a screen on a cable it is not so (below). Not yet tried with the
      holder taking up its own new version between the two compositors,
      which hands the card on as well.
- [x] A change of compositor unseen, the third step: the picture before,
      read from the card as each screen shows it, drawn over the session's
      first frames and faded out in a quarter of a second, once the
      session is shown. Tried on a laptop's own screen.
- [x] A change of compositor with a screen on a cable less than half
      as slow, found by hand on 17 November 2026 with one on HDMI beside
      a laptop's own: the session stood still for 2863 ms where it does
      for 811 ms without it. The next compositor asked every socket of
      the card anew what was in it, 1.2 s for the screen on HDMI and 0.8
      s for the empty socket beside it; no mode was set anew. Taking
      the screens as the one before left them, it now takes what the
      kernel last found, and asks only where the kernel does not know
      (ADR 62): the same change stands still for 1207 to 1272 ms. At
      login, and as a screen is plugged in or pulled out, it asks as it
      did, which is when the kernel says to.
- [ ] The same change, what is left: with the screen on HDMI the next
      compositor still takes 636 to 697 ms to come up, where it takes
      266 ms with the laptop's screen alone, and where those 400 ms go
      is not known. The backend is to say what it spent on each screen
      when it is slow to come up.
- [ ] The same change, what is not explained: the screen on the cable
      went black for about a second the first time, and in none of the
      five changes watched after, three with the sockets asked anew and
      two without; and a
      window of foot came back at another size, once. Asking a socket
      anew may make a screen flicker, the kernel says, so the first may
      be gone with it. Waits on: either happening again.
- [x] The session never out of anyone's control (ADR 63): `omorfos-hold`
      takes the seat once, through libseat, and keeps it while compositors
      come and go, opening the cards and input devices for each and closing
      them as it ends, keeping the first card open for its picture; the
      compositor takes its devices from the holder through Smithay's
      `Session` trait, and its own seat where the holder keeps none. With it
      the virtual terminal is not put back into text between two
      compositors. Tried on real hardware, logged in at the console.
- [x] The holder taking up its own new version only where it is another:
      installing always puts a new file in place of the old, and the holder,
      which lets the seat go to take up a new version, showed the console
      then, once each installation; it now takes up the file installed only
      where what is in it is not what it runs.
- [x] Blue and green, once the three steps are tried, decided against (ADR
      62, "Alternatives"): measured on real hardware, a change of
      compositor stands still for 600 to 850 ms, with the picture before on
      the screens and a fade after, which a compositor caught up beside the
      one going, the relay feeding both, would not be worth its cost to
      shorten. The holder's log says where the time goes, should it grow.
- [x] Changes of compositor tried with real programs, kept in the tree and
      run by hand (`real.rs`, `docs/TESTING.md`), where the probes so far
      were thrown away: foot, Firefox ESR, a GTK program and a Qt program in
      a Flatpak, through three changes one after another, each checked to
      be kept, running, and drawing again at a new size.
- [x] Real programs in CI: foot and the GTK program on every push, from
      Ubuntu's archive, and Firefox ESR, from Mozilla's repository, and
      Telegram, from Flathub, with them every night.
- [x] The same for the components, which the compositor starts with their
      roles: the panel, the wallpaper, the notifier and the clipboard's
      keeper, kept as the same processes through each change, the panel
      hearing the next compositor over IPC (`docs/TESTING.md`, "Real
      programs").
- [x] A routine for real hardware in `docs/TESTING.md` ("A change of
      compositor on real hardware"): one screen, a window fullscreen on
      another workspace, two screens, one on a cable, another virtual
      terminal, locked, a video playing, a window of a program written for
      X11, and installing anew, with what is to be seen in each and what
      the log says.
- [x] Ghostty ended as the compositor changed, with thirteen windows
      open, on 23 November 2026: GTK before 4.24, told a second time what
      buffers of the graphics card it may make, lets go of what it should
      not, and ends (`docs/UPSTREAM.md`, under GTK). The next compositor
      tells each feedback a kept program asked for, made again, what the
      one before did. The relay now holds that back while it is as the
      program was told, comparing tables of formats by what they hold,
      and passes it on where it is not; a feedback that changed, as after
      the driver is updated, still reaches the program, and a GTK older
      than 4.24 may end on it.

## 4. The look

Stage 2 says what it is to be; this builds it. The redrawn theme and
`appearance` come first, then the shadows, then glass, as more waits on
the first than on the last.

- [x] Shadows under windows, menus and panels (`box-shadow`), as
      pictures made on the CPU (ADR 33): each shadow made once, blurred,
      and cut into nine pieces, kept while it is in use; under windows,
      the compositor's menus and the components' panels, changing with a
      window's focus and fading with what goes.
- [x] The built-in theme's shadows, at the levels of `docs/DESIGN.md`
      ("Depth"), with the focused window's edge in `--border-strong` in
      place of the accent; the tests that take the space beside a window
      or a menu to be bare made to say what they mean: it is the
      background, or the background in a shadow.
- [x] Each notification a panel of its own, so that each casts a shadow
      round itself and comes and goes on its own, put in its place by
      the margins of its layer, all of them on the screen the first was
      put on; those showing make room for a new one only once it is
      drawn; the shadows of panels on one layer are drawn under all of
      them, so that one does not fall on the next; and the built-in
      theme gives `notification` the second level's.
- [x] A shadow under the questions `omorfos-ask` puts, as under a
      dialog: the prompt marks its box by the region it takes the pointer
      in, which moves with it as it shakes, and the compositor casts the
      `question` part's shadow round that, at the second level.
- [x] A shadow under what the portals ask, as under the prompt for a
      password: the box's panel is named after the `question` part, whose
      shadow it casts, and a question comes and goes at once, whatever
      the part's `transition`, so that it is not in the picture it leads
      to; the pictures a colour is picked from are named after no part.
- [x] Pictures of a window by itself in its frame, with the shadow it
      casts, as a program that draws its own titlebar has its own in its
      picture: shared and saved alike, as it would be shown wherever it
      is, the tab in front if it is behind another; one size whether it
      has focus or not; and the picker outlining the frame, and showing a
      covered window by itself as far beyond that as its picture reaches,
      which the socket for scripts says.
- [x] Shadows drawn in software whole to their ends: the software
      renderer stretches a piece of a shadow's picture with a bilinear
      filter and nothing beyond its edge, so a side of a shadow faded
      toward its corners, to about a third at the ends of the password
      prompt's sides. Each piece now carries a border copied from its
      edge, as GLES clamps to it; the fault in Smithay is in
      `docs/UPSTREAM.md`.
- [x] A shadow's picture made faster: it is made each time what casts
      it comes, in the frame it opens in, and its softening took nearly
      all of that, each pixel adding its taps one by one, down the
      columns across rows of memory. Each tap is now added to a whole run
      of pixels at a time, along rows, to the same sums in the same order,
      so the picture is the same to the bit: a menu's is made in 0.85ms
      at a scale of 1 where it took 9.2ms, and in 5ms at a scale of 2
      where it took 60ms, in an optimised build.
- [x] Shadows under the compositor's notice, tooltips and the palette of
      tiles, which are drawn by the compositor and are not a part's
      layer: the notice's fades with it, and the tooltip's falls under
      the mark it tells of, which nothing darkens.
- [x] A shadow under the switcher on Alt+Tab, at the second level with
      the launcher, which it floats over the windows as: cast round the
      whole of it, under the windows shown small in it as well as its
      own pixels, kept while it is not painted anew and gone with it.
- [x] Glass, drawn in software and tested (`backdrop-filter: blur()`):
      what is under each sheet, its blurred picture, when that is stale,
      and the software blur that is the shader's specification. The
      wallpaper is blurred once and used wherever nothing else is under
      the glass. The hardware backend draws solid until the next item.
      Under menus, the pictures of arrangements, the notice, the switcher
      and the components' panels; solid with
      `appearance.reduce_transparency`; how many blurs were made is kept,
      for the state published over IPC.
- [x] Known gaps in glass: a menu, the notice or a panel that has gone
      keeps its glass as it fades, told apart from one like it that has
      come since; the box that asks for a password is glass as the theme's
      `question` is, blurring the dim and the session under it, and the
      tooltip of the mark as `tooltip` is.
- [x] The blur shader, its check at startup, and the file left out of
      coverage, tried on real hardware: behind the bar, the dock, menus
      and notifications, for the compositor's parts and the components'.
- [ ] The theme's elements in a tree, as CSS names them (ADR 110,
      #183), before 0.1.0: every part an element at one path, `button`,
      `input` and the rest the same element wherever they are; types,
      classes, states on any element, `::backdrop`, `a b` and `a > b`,
      and CSS Nesting; matched once as the theme is loaded, a selector
      that reaches nothing an error; `color` and the font inherited;
      the built-in theme, the guide, the manual page and what is
      promised renamed at once.
      In three steps, the built-in theme the same, pixel for pixel,
      through them all (ADR 110, addendum):
      - [x] The syntax: the tree, selectors, nesting, specificity, the
            web's states, matched once as the theme is loaded, a
            selector that reaches nothing an error naming what is near,
            and every theme renamed (#192).
      - [ ] States at any depth, as `window:focus-within titlebar
            controls button`, an element's looks capped at 4,096.
      - [ ] `color` and the font inherited, `:root` an element above
            every surface, a `dialog` for each program that draws one,
            a notification's own `button`, and `::selection` an element
            of its own rather than a state of its field.
- [ ] The looks of the built-in theme unified, once ADR 110's three
      steps are in: `button` one look wherever a button is, and the
      record of every look the steps kept the same let go.
- [ ] The tree of the theme's elements printed, each with its states,
      as a browser's inspector shows a page's (`omorfosctl theme tree`),
      for whoever writes a theme.
- [ ] The check's allowance, 4 in 255, measured on cards other than
      Intel's, where the shader is within 2: AMD's and NVIDIA's.
- [x] Glass at the edges of the screen: beyond them is what is inside,
      mirrored, as CSS has it, and no glass is blurred less than 16 px,
      so that a bright line along the bottom of a wallpaper is a glow
      under the bar, not a line.
- [x] How long a frame took to draw, and how many blurs it made, over
      IPC: not in the state, which would then be sent to every subscriber
      with every frame, but as a request of its own, `"frames"`, as
      `"placed"` is, and `omorfosctl frames`. The time is the compositor's
      own, until the frame is handed to the graphics card.
- [x] Blur behind titlebars, where the theme makes `titlebar` glass:
      under each, rounded above as its window is and square below, kept
      as the window rolls up into it and as it fades out. The built-in
      theme does not, so it costs nothing until a theme asks for it.
- [x] What glass behind titlebars costs, measured with `omorfosctl
      frames` on a modest laptop with a dozen windows, one dragged over
      the others, and written into ADR 33. It cost a frame, 17 in 447 on
      a UHD 620 at 3840 by 2160, until what is behind glass was blurred
      from a picture half as large and a sheet left alone while nothing
      under it changes; then none in about 1600, the slowest 14 ms, and
      glass stays. The built-in theme's titlebar
      is glass since ADR 97, ahead of that, as the maintainer asked, and
      is made solid again if it costs a frame. Everything that floats
      and the bar followed it the same day, as asked, so what is to be
      measured is a desktop of glass: the bar, which is always there,
      first. What the review of 19 November 2026 found glass doing more
      than once (findings 6, 7 and 13, in stage 8) is taken as these
      numbers say.
- [ ] What a glass titlebar allocates where nothing changes, brought
      down: 7 times a turn for each, counted by the tests of a quiet
      session beside the 13 of a window in a solid frame, in laying its
      sheet out, in looking at what is under it and in cutting it to its
      corners. Both are to come down to nothing.
- [x] One hairline and one blur for every surface (ADR 97): `--edge`
      round a window and everything that floats, a hairline seen
      through by default, light in the dark scheme and navy in the
      light, and `--blur` wherever there is glass, each changed in one
      place and by a part's own rule for that part alone.
- [x] Glass by default on what else floats, in `--glass-thick` and
      `--blur` as titlebars have it (ADR 97): menus, tooltips, the
      notice, notifications, the launcher and the welcome card drawn as
      it is, the switcher, the bar and its popovers; asked for by the
      maintainer on 18 November 2026. Each held to 4.5 to 1 over an
      ordinary ground and 3 to 1 over the worst that can be behind it,
      the dark scheme's glass at 80 in a hundred and the light's at 85,
      thinner than the 90 and 94 a stricter rule first made them; the
      notice's is 95. The bar's glass keeps within
      the room its theme gives it. The tests' screen blurs only where a
      test asks.
- [x] Glass behind pop-ups (#119): the bar's popovers were drawn see
      through with nothing blurred behind them, as were the menus of
      programs that asked for blur. A pop-up of a component's panel is
      the `popover` part, with its glass, its shadow and its coming and
      going; any program's pop-up has what it asks for through
      `ext-background-effect` or KDE's protocol, cut to the pop-up
      proper. What is behind a pop-up is all that is under it, the window
      or panel it hangs from among it.
- [ ] Known gaps in a frame whose theme takes its edge away: the
      window's bottom corners are cut to the curve with nothing drawn
      along it, so they are not smoothed; and a frame at the edge of
      the screen cannot be held by that side to resize it, the room to
      hold it by being outside it.
- [x] A portal that starts says every setting, once, to the programs
      already running: one started again with other settings than the
      one before it had, as after an upgrade that changes the built-in
      theme, left them with what they had read, the side of a titlebar's
      buttons among it, until they were started again.
- [x] Blur for other programs that ask (`ext-background-effect`), under
      windows and panels, as far as `window`'s `backdrop-filter` says, cut
      to the surface, a window's to the window proper and not the shadow
      it draws round itself, and to the frame's rounded corners, one blur
      for all of a region; kept through a change of compositor by the
      relay.
- [x] The same through KDE's protocol (`org_kde_kwin_blur`), which
      Ghostty, KDE's programs and fcitx5 ask through rather than the
      standard (ADR 64).
- [ ] Take KDE's blur protocol out once Ghostty, KWindowSystem and fcitx5
      ask through `ext-background-effect`: ADR 64 says how to tell and
      what goes.
- [ ] Known gaps in blur for other programs: a pop-up's, or a part of a
      window drawn on a surface of its own, is not blurred behind; a
      window's goes as soon as it closes, rather than fading with it; and
      every program is blurred by as much, which a rule cannot change; a
      window that draws its own frame with rounded corners, as GTK's do,
      and asks for all of it, as Ghostty does, is blurred into the
      corners, as the compositor knows not how round they are.
- [x] A transition's delay, in `transition` and `transition-delay`, and
      `cubic-bezier()`: a part waits as long as its theme says before its
      colours change, and fades as it comes and goes after the same wait.
- [x] Gradients as a background, `linear-gradient()`, over the part's
      colour, wherever a part is a box, its titlebar among them; glass
      under one that can be seen through.
- A picture as a background (`url()`) is set aside, to be built only
  once a design needs one. It would be the first time a theme had omorfos
  read a file the theme names, and every process that draws parts would
  decode it: a JPEG decoder in the compositor, the panel, the
  notifications, the launcher and the prompts, where only the wallpaper
  and the lock screen have one now, and a table of pictures for each
  theme carried to every place a part is drawn, as a part's style, copied
  freely, cannot hold one. Were it built: only from the configuration's
  directory, covering its part, decoded by what the wallpaper decodes
  with and with its limits.
- [ ] Known gaps in gradients: `radial-gradient()`; a gradient on the
      desktop and the dims, which take its colour halfway; and a gradient
      that changes with the part's state, which changes at once rather
      than over its `transition`.
- [x] The built-in theme redrawn in the design language, in its dark
      scheme: its colours, the corners of each part, rows and fields as it
      has them, padding on its grid, a row under the pointer raised and
      one chosen in the accent, and its contrast tested; solid, until
      glass has been measured. Every window's edge is a hairline, with
      focus or without, as programs that draw their own have it; and the
      edge of a frame follows its top corners round, as it did its bottom
      ones.
- [x] The light scheme, and the accent a setting: `appearance.scheme`
      and `appearance.accent`, laid over the built-in theme as a few
      variables, under the user's own theme; each accent that comes with
      omorfos held to the contrast in both schemes, and programs told
      the scheme and the accent through the settings portal.
- [x] A theme that is wrong keeps the scheme and the accent it was last
      built with until it is mended, as the whole of it is kept: a new
      scheme in `config.yaml` meanwhile waits for it. Where nothing of the
      user's was kept, as at startup, it is the built-in theme in the
      scheme chosen.
- [x] The ring round what the keyboard is on, two pixels of the accent
      two pixels out from it (`outline`, `outline-offset`), on the
      prompts' buttons; close round the eye in the password field, which
      has no room round it.
- [x] The session blurred as well as dimmed under the prompt that asks
      for a password (`sealed`'s `backdrop-filter`), by default; the dim
      alone where blur cannot be drawn.
- [x] What arrives taking longer than what leaves: a part comes over
      its own `transition` and goes over the one it has `:closing`; the
      built-in theme brings windows, menus, the notice, tooltips, the
      components' panels and the lock screen's question in over 180 ms
      and takes them out over 120 ms, as DESIGN.md's "Movement" has it.
- [x] What arrives grown from 96% of its size and leaves shrunk to 98%,
      as DESIGN.md's "Movement" has it: `@starting-style` and `scale`,
      as CSS has them, and how see-through and how large a part is while
      it is `:closing`, drawn as a picture made larger or smaller, its
      shadow and glass with it (ADR 25).
- [ ] Known gaps in coming and going: a part's colours stay as they are
      while it is `:closing`; `scale` only as a part comes and goes, not
      at rest; the tooltip appears and goes at once, and the switcher
      goes at once; and
      the corners of a part's glass are as round at every size.
- [x] The inner hairline of light along the top of glass: a sharp
      `inset` shadow in `box-shadow`, beside the one cast, and
      `--glass-rim` for it in the built-in theme, white at 6% in the dark
      scheme and none in the light; on nothing until glass is.
- [x] `appearance.scheme: auto`: light from sunrise to sunset, worked
      out from a latitude and longitude the user gives, or fixed hours
      without one (ADR 65). Each program works it out, and wakes to load
      the configuration again when it may change.
- [x] `scheme: auto` where the user is without being told: a location
      may be a time zone by name, or `system`, the place of its principal
      city, and is the system's unless a location or hours are given.
- [ ] Known gaps in `scheme: auto`: the lock screen, the box that asks for
      a password and the launcher keep the scheme they were opened in; a
      change of time zone is only seen at the next midnight or the next
      change; the built-in table of time zones is tzdata 2026f's, to be
      brought up to date with tzdata; and the system's time zone as where
      the user is, and 07:00 to 19:00 where it has no place, are defaults
      to review.
- [x] Day turning to night as a transition: at sunrise and sunset the
      colours of what the compositor, the panel and the notifications draw
      change over a minute, a step a quarter of a second, rather than at
      once; programs that follow the desktop, through the portal, are told
      as it begins (ADR 65). A scheme changed by hand changes at once.
- [ ] Known gaps in day turning to night: what goes by the scheme rather
      than by a colour changes as it begins, the pointer and the mark in
      "About" among them; and a program that draws itself, as GTK's do,
      changes at once, as it is told.
- [x] A pointer that is the same size everywhere, including over
      programs written for X11: its theme and size in the X server's
      resources as well as in the settings toolkits follow, told again
      when the cursor changes and not only the scale; and through the
      settings portal's `org.gnome.desktop.interface`, for GTK in a
      sandbox. A cursor theme is a directory's name.
- [x] The default font: Inter in the built-in theme where it is
      installed, `font-family` taking a list of families as CSS does;
      told to programs in the settings toolkits follow under X11 and
      through the settings portal; and recommended by the packages, with
      JetBrains Mono.
- [x] The icon theme: `icons.theme`, Adwaita unless another is named,
      used for the icons omorfos looks up, then the themes it inherits
      from, then `hicolor`, as the icon theme specification has it; and
      told to programs as the font is.
- [x] Programs told the monospaced font, JetBrains Mono: the built-in
      theme's `--font-monospace`, told through the settings portal's
      `monospace-font-name`, which libadwaita and the editors and
      terminals built on it read. Programs written for X11 are not told,
      as no setting of theirs says it.
- [x] A pointer of our own, built in as shapes so that it is sharp at any
      size and there when no cursor theme is installed: `cursor.theme:
      omorfos`, the default, navy or cream as the scheme is, and
      installed as the cursor themes `omorfos-dark` and `omorfos-light`
      for programs that draw their own (ADR 66), which are told in
      `XCURSOR_PATH` where to find them.
- [ ] Known gaps in the pointer: the ring of waiting does not turn,
      neither drawn by the compositor nor in the themes on disk, which
      hold one picture of each shape; a program that read
      `XCURSOR_THEME` or `Xcursor.theme` as it started keeps the scheme
      it started in.
- [x] Packages carry the licence texts `CREDITS.md` names, Space
      Grotesk's OFL among them, and those of the crates omorfos is built
      from, gathered by cargo-about as it is packaged (ADR 67).
- [x] "About" shows `CREDITS.md`: `about`, in the root menu, shows a
      panel in the middle of the screen with the mark, the version and
      the licence, and each thing of someone else's with its licence and
      whose it is, as `CREDITS.md` was when omorfos was built; glass, and
      put away by Escape, a click beside it, or `about` again. It had a
      window's close button too, taken out on 19 November 2026 as the
      welcome card's was: neither is a window.
- [x] "About" on a screen too short for it: no taller than the screen
      within its margins, what it lists scrolled by the wheel, the
      touchpad, the arrows, the keys of a page, space, Home and End,
      without being painted again. It has the keyboard while it is
      shown: no window sees a key, a binding still works; and it is put
      away when the session is locked.
- [x] Titlebar buttons at the right, close outermost, as most desktops
      have them; and programs that draw their own titlebars told the
      same: GNOME's `button-layout` through the settings portal, which
      GTK, libadwaita and Qt's Adwaita titlebars read, in Flatpak too,
      and `Gtk/DecorationLayout` in the settings programs written for
      X11 follow. A button the theme leaves out is left out of both
      (ADR 68).
- [ ] Known gaps in where titlebar buttons stand: programs that draw a
      titlebar of their own making, as Electron's do, are not told; nor,
      it is thought, GTK 3 under Wayland outside a sandbox, which reads
      GNOME's settings itself. freedesktop's own setting for it, a draft
      in xdg-desktop-portal (pull request 1821), is to be served too
      once it is agreed.
- [x] Icons in menus: a sign from Lucide before each entry of the
      titlebar's menu and of the root menu, a program's own icon before
      one that starts it, or one `menu.yaml` names; and the sign of what
      is on in the accent, on a faint square of it, in place of wording
      that changes; and a submenu lying a little over its menu (ADR 70).
- [x] The root menu's Terminal row as its others (#120): `open:
      terminal`, the first terminal an `xdg-terminals.list` names, as
      `xdg-terminal-exec` reads it, and `foot` where none is, with
      Lucide's `terminal` sign, where it showed foot's own icon in full
      colour beside the signs of the rows round it; and `Super+Return`
      with it, which the row shows again. A row of `menu.yaml` that
      starts a program keeps that program's icon.
- [ ] Known gaps in icons in menus: a program started from the menu is
      found by its file's name, not by the desktop entry whose `Exec`
      starts it, so that `flatpak run` and Ghostty need an `icon`; a
      change of icon theme is seen in an open menu only once it is drawn
      again. The root menu stays the user's to arrange in `menu.yaml`,
      `icon` with it, once the bar has a menu of its own, which is the
      system's, fixed (ADR 108, stage 5).
- Movement. Windows that open and close with a slight change of size as
  well as of opacity, and workspaces that slide, were there already
  (above, and in stage 3). The rest, in this order:
  - [x] All of it off with one setting: `appearance.reduce_motion`, a
        theme still whatever it says, nothing sliding or shaking, and
        programs told, through the settings portal and to those written
        for X11 (ADR 71).
  - [x] A notification that comes in and goes beside others that stay,
        and those below sliding up into the room one leaves, where they
        jumped: each glides to its new place over 240 ms, as DESIGN.md
        has something moving to where it was sent, unless the theme has
        notifications change at once or motion is to be reduced.
  - [x] The panels and the titlebar sliding in from the edge they are
        brought back at on a fullscreen workspace, and out again: as one,
        over the theme's `panel` transitions in and out, 180 ms and 120 ms
        in the built-in theme, and at once with motion reduced (ADR 36).
  - [ ] Known gaps in what slides in at an edge: with a screen above
        another, what slides in from the top of the lower one is seen for
        a moment at the bottom of the upper; and the titlebar on its way
        out can still be clicked, where the panels on theirs cannot.
  - A window that changes size, by its edge, by the line between
    parts, by being tiled, maximized or given the room of one that
    closed, changing it smoothly, in this order (ADR 72):
    - [x] A program told nothing new about its window until it has drawn
          what it was told last, and then the latest, so that a slow one
          is not asked every size an edge is dragged through.
    - [x] The window's last picture stretched to its frame until its
          program has drawn the new size, then faded into it over
          `window`'s transition, where windows change over a time.
    - [ ] Known gaps in a stretched picture: the pointer is in the window
          where it would be unstretched; a program written for X11 is not
          stretched, nor told in a way that is answered; and a window's
          pop-ups are stretched with it where its frame is square and not
          where its corners are round.
    - [x] Tiling, maximizing and filling the room of a window that closed
          easing the frame there over 240 ms, its program told only where
          it goes, and what is dragged following the hand at once.
    - [ ] Known gaps in a frame easing: going fullscreen and back is not
          eased, nor is a window sent to another screen; a program that
          draws on the way is shown at once, not faded into; and a line
          is taken hold of where it is going, not where it is drawn.
    - [x] The line between parts easing when it moves by itself: put back
          in the middle, with the windows either side of it, or cutting
          more of the screen once a part has closed.
    - [ ] A program written for X11 told at once still: it could be asked
          to say when it has drawn (`_NET_WM_SYNC_REQUEST`) once Smithay's
          window manager offers it.
  - [ ] A window that goes to the dock when minimized, once there is a
        dock (stage 5).
- [ ] To be tried once there are effects: on a shared workspace, the
      windows without focus drawn a little fainter or blurred, so that
      the one with focus stands out. It has to be weighed against the
      design language, which keeps a window's content from being glass
      because it is there to be read.
- [x] The switcher as a queue on a diagonal (wish 5), unless
      `switcher.style: flat` (ADR 75): the one picked in front, its title
      under it, those after it smaller, fainter, up and to the right, and
      the queue turned over 240 ms as the next comes forward.
- [x] The switcher's screen dimmed and blurred behind it
      (`switcher-backdrop`), and on a diagonal each window's border round
      its picture rather than a box behind it (`switcher-window`).
- [x] The switcher shown only once Alt has been held 200 ms after Tab, so
      that a quick `Alt+Tab` goes back to the window before without it,
      and fading in over 120 ms, with its dim and blur.
- [x] A private window in the switcher as its rule has it in pictures:
      blurred with `private: blur`, and blank otherwise, in the `capture`
      part's colour.
- [x] A private window whole in the switcher and the overview on the
      screen, and in pictures of the screen blank or blurred without its
      title (ADR 84).
- [ ] A private window offered in a part of a workspace with nothing in
      it shown whole on the screen too, and without its title in a
      picture, as in the switcher (ADR 84): it is blank on the screen
      still, and its title is in pictures of the screen, which it should
      not be.
- [ ] Known gaps in the switcher on a diagonal: the borders further back
      are thinner, being drawn smaller; a theme's `box-shadow` on
      `switcher-window` is not drawn; the pictures are not rounded at the
      corners; and it goes at once, with the dim and blur behind it,
      rather than fading out as it fades in.
- [x] An overview of every window on a workspace, and of the workspaces
      (ADR 76): an `overview` action, the windows of the workspace laid
      out small, gliding there from where they are, the one picked by the
      pointer or the arrows, and a strip of the screen's desktops as live
      miniatures to go to, with a plus after them that makes another; and
      three fingers swiped up the touchpad carrying it open, down closed.
- [ ] Dragging a window in the overview onto a workspace in its strip, to
      move it there; and onto the plus after the strip, to make a new
      workspace and move it there in one, the plus staying, the new
      workspace not being empty (asked for by the maintainer on 26
      November 2026).
- [ ] Known gaps in the overview: the windows of a workspace left in it
      go at once rather than glide out; which window is over which on a
      workspace not shown is the order they were made in; a window's
      titlebar is back only once it has glided home; the miniatures show
      windows without their frames; a fullscreen window's workspace is not
      in the strip; `Super+Right` while it is open goes along the row of
      the screen the pointer is on, not the overview's; three fingers
      swiped across do nothing while it is open; and a screenshot taken
      while it is open is of the window with focus, not of the overview,
      as the maintainer found on 15 November 2026.
- [x] Night light: the screens warmer after dark (ADR 74), by the day
      and night `scheme: auto` goes by, turning over twenty minutes either
      side, through each screen's gamma table; `night_light` in
      `config.yaml`, off unless set, and an action to switch it until it
      next turns.
- [ ] Known gaps in night light: it is set in the hardware backend,
      which no test runs, and tried by hand on a laptop's own screen; programs
      that set a screen's gamma themselves, through
      `wlr-gamma-control`, as `gammastep` and `wlsunset` do, are not
      offered it; nothing on the panel says it is on; and on an HDR
      screen (stage 8) it is to warm the picture as it is composed
      instead.
- [x] Windows redesigned (ADR 97): omorfos's own frame, on the windows
      that ask for one, at home beside what programs draw for
      themselves, which are told what every desktop tells them and
      neither themed nor framed by force. The titlebar 48 tall and thick
      glass; the buttons 16 across at the left, close, minimize,
      maximize, red, amber and green in both schemes, all three showing
      their signs while the pointer is on any; a title alone in the
      middle; a fainter hairline round the window. A program that says
      it draws its own titlebar is left to, and GTK 3 is told to read
      the portal. Settled by the maintainer in a mock and on real
      hardware, on 18 November 2026.
- [ ] What is left of the redesign of windows, decided on 24 November
      2026; Google Chrome puts its buttons where it is told, as tried on
      18 November 2026:
      - [x] "About" says how it is put away, in a faint line of help at
            its foot, translated, as the screenshot picker's is: Escape
            or a click outside. The welcome card, which has Skip and
            Finish, stays as it is (#137). The line is in the middle, in
            `about-detail`, and stays where it is while what is listed
            above it scrolls. With it, once tried, the overview's button
            that closes, on the card's top left corner while the pointer
            is on the card, which no other card had when this was decided
            (ADR 69). It, and the overview's, fade in and out as a menu
            does, by `Motion::fade`.
      - [x] The switcher's pictures show a window in its frame, as it
            will be when chosen, every program alike; as a window alone
            is drawn for pictures of it. In rows and in the queue, the
            frame omorfos draws, titlebar, buttons and title, is made as
            small as the picture with the window, its glass with it; a
            program that draws its own frame is pictured as it draws
            itself. A private window in a picture of the screen keeps no
            frame, as its titlebar would say what it is (#140).
      - [ ] The side of a titlebar's buttons a setting,
            `appearance.buttons`, `left` or `right` (ADR 107).
- [x] A theme able to style the button under the pointer apart from the
      other two (ADR 105): the three are in `windowcontrols`, `:hover`
      while the pointer is on any of them, and each is `:hover` only
      under it; a rule names the controls before a button, as CSS writes a
      part within another, `windowcontrols:hover button-close`, and the
      built-in theme does, so that it looks as it did.
- [ ] Titlebars with controls in them, for omorfos's own programs (stage
      7), as GNOME Files and the Finder have: back and forward, where
      one is, a field to search in, where a plain window has its title.
      Each draws its own, in the look of ADR 97 and taller by what a
      control needs; built with the first of them, and with it what
      they share to draw it by.
- [ ] The defaults reviewed together, keys and settings, rather than one
      at a time as each feature came. What has gathered for it: `Super+l`
      to lock; a key for the overview; `Super+c`, `x`, `v` and `z` to copy, cut, paste and undo,
      and with `Shift` too, which needs an action that gives the program
      with focus its own keys for them; keys to tile; whether
      `Shift+Print` keeps its picture as a file rather than on the
      clipboard; the star key (`XF86Favorites`); a key for
      `next_layout`; the keys for the volume, the brightness and what is
      playing; `XF86Display` for `next_screens`; one of `Ctrl+A` and
      `Super+A` to choose all that is typed in a field for a secret, not
      both, with the keys to copy and paste; whether `clipboard.forget`
      forgets by default, and after how long; the launcher's icons and
      descriptions, shown by default since 15 November 2026, as asked; and
      the theme's parts made along the way. Reviewed on 20 November 2026,
      against GNOME, KDE, macOS and Windows
      ([the review](reviews/2026-11-20-the-defaults.md)), with a proposal,
      the Mac's keys first with Super for Command, for what it went
      through in [ADR 101](adr/0101-super-drives-the-desktop-and-the-ordinary-way-is-a-setting.md),
      for the maintainer to decide; the theme's parts are left to the
      design review. Decided on 24 November 2026: ADR 101 accepted, but
      for the next layout and the root menu having no key by default, and
      reloading the configuration none; `Super+F1` beside the Display key
      for the screens; the question's minute and the tooltip's half
      second fixed, as they are; `Super+Return` and `Super+Ctrl+N` as
      they are. To build, as ADR 101's "As accepted" says, in the next
      item.
- [ ] Super as the key omorfos is driven by, everywhere, and a way back to
      Ctrl and Alt as Windows, GNOME and KDE have them (asked on
      15 November 2026). ADR 78 gave Super to the desktop, and hands a
      chord with Super that nothing binds to the program as Ctrl, so that
      `Super+C` copies as Cmd+C does on macOS; but what the other desktops
      put on Ctrl and Alt is still there beside it. `Ctrl+C`, `Ctrl+V`
      and the rest reach programs as they are typed, and do in the shell's
      own fields too (the launcher's, the boxes that ask for a password);
      `Alt+Tab` switches windows as `Super+Tab` does; `Alt+Print` takes a
      picture of a window where `Print` takes one of the screen. To do:
      - every common chord gone through and found on Super, the built-in
        bindings and the shell's fields alike: switching windows, closing
        and quitting (`Super+Q` closes the window today), pictures of the
        screen and of a window, the launcher, locking, the overview,
        copying, cutting, pasting, undoing, choosing all, finding;
      - decided, for each, whether Ctrl or Alt keeps doing what it did as
        well, or is left to programs alone, which takes `Alt+Tab` from
        the desktop and gives the shell's fields `Ctrl+C` no more;
      - a setting between the two ways, omorfos's with Super and the
        ordinary one with Ctrl and Alt (`Alt+Tab`, `Alt+F4`, `Ctrl+C` as
        typed everywhere, Super left to the launcher and the overview),
        its name and place in `config.yaml` to be decided, taking effect
        at once, and perhaps asked on the welcome card;
      - decided with the review of the defaults above, as an ADR that
        takes up ADR 78, and tested chord by chord in both ways.

      Decided on 24 November 2026, in ADR 101 as accepted: the Mac's
      keys, Super for Command, and `defaults: ordinary` in `keys.yaml`
      for the other way. To build, each tested chord by chord in both
      ways:
      - [x] the seven new actions, `switch_programs`,
            `switch_programs_back`, `next_window`, `previous_window`,
            `close_program`, `hide_program` and `hide_others`, the
            switcher showing programs (#130, #131): a program is its
            windows that share an application's name, and one with none
            a program of its own; not bound yet;
      - [x] the built-in bindings moved as ADR 101 has them, and as it
            was accepted: no key for the next layout but `XF86Keyboard`,
            none for the root menu, none for reloading, `Super+F1` for
            the screens (#132); pictures of the screen as a Mac's,
            `Super+Shift+3` and `4` kept, and with Ctrl on the clipboard,
            `Super+Shift+5` the picker; Alt+Tab left for the ordinary way.
            the program's windows on `Super+Ctrl+Down` (#141);
      - [x] Super with the arrows for text, as a Mac's Cmd, handed to
            programs as Home, End, Ctrl+Home and Ctrl+End and read so by
            the shell's fields, Super+Backspace there taking out to the
            start; the desktop's arrows on Super+Ctrl, the overview on Up,
            the workspaces on Left and Right, between tiles on
            Super+Ctrl+Alt (#155). An arrow with a modifier held is a
            binding's while the overview is open, so Super+Ctrl+Up closes
            it. Super+Ctrl+Down shows the program's windows (#141);
      - [x] the overview with one program's windows alone, as App Exposé
            shows them: `show_program_windows`, on Super+Ctrl+Down in
            the Mac's way and on no key in the ordinary way, closing the
            overview while it is open whichever windows it shows (#141).
            The four-finger swipe down opens it too (#138);
      - [x] the shell's fields reading Super as Command, Ctrl left for
            the line (#133): the launcher, the boxes that ask and the lock
            screen copy, cut, paste, choose all and undo with Super, or
            with Ctrl in the ordinary way; the compositor hands them Super
            as it is, as it does no other program. What a Mac's fields do
            with Ctrl, as Ctrl+A to the start of the line, is to come;
      - [ ] the touchpad's three or four fingers, a pinch for every
            program and a spread for the desktop; a page of settings for
            the gestures, for stage 7;
      - [x] `defaults: ordinary`, and the release that moves the keys
            saying which (#133): those ADR 101 names, and with Alt and
            Ctrl GNOME's where it names none, `Alt+F10` maximizing and
            `Ctrl+Alt+Delete` logging out; Super holding only what ADR
            101 names (#153), every other chord with it reaching the
            program as it is.

The review of 26 November 2026, of the design, the brand and the shell
for accessibility, is written up in
[reviews/2026-11-26-accessibility-design-wcag.md](reviews/2026-11-26-accessibility-design-wcag.md),
and [#180](https://github.com/omorfos/desktop/issues/180) keeps its
findings as a list to tick. Those that are of the look are here, in the
order to take them, before the bar and the dock wear it; the decision
on assistive technology is at the head of stage 5, and the rest are in
stage 8, under "Accessibility":

- [ ] Medium: the contrast test a list kept by hand, and DESIGN.md's questions
      of a new part not asking
      ([finding 15](reviews/2026-11-26-accessibility-design-wcag.md#15-the-contrast-test-is-a-list-kept-by-hand-and-the-checklist-does-not-ask)): each part saying what it is written on, the
      test walking `Part::ALL`, and six more questions.
- [ ] Medium: dim text on glass under 4.5 to 1 at its worst
      ([finding 1](reviews/2026-11-26-accessibility-design-wcag.md#1-dim-text-on-glass-is-under-45-to-1-at-its-worst)): 3.6 to 1 over a
      white window in the dark scheme, 3.8 over a black one in the light.
- [ ] Medium: an accent of one's own taking away the focus ring and every
      lit row
      ([finding 2](reviews/2026-11-26-accessibility-design-wcag.md#2-an-accent-of-ones-own-can-take-away-the-focus-ring-and-every-lit-row)).
- [ ] Medium: states said by marks under 3 to 1, and a window that wants
      attention not marked at all
      ([finding 11](reviews/2026-11-26-accessibility-design-wcag.md#11-states-said-by-marks-under-3-to-1-and-one-not-said-at-all)).
- [ ] Low: the faint colour used for the days of other months
      ([finding 16](reviews/2026-11-26-accessibility-design-wcag.md#16-the-faint-colour-is-used-for-text)).
- [ ] Low: an offered window's title written straight onto the wallpaper
      ([finding 17](reviews/2026-11-26-accessibility-design-wcag.md#17-an-offered-windows-title-is-written-straight-onto-the-wallpaper)).
- [ ] Low: no look for disabled, pressed, chosen as against where the
      keyboard is, more below, or a chain of keys waiting
      ([finding 22](reviews/2026-11-26-accessibility-design-wcag.md#22-states-the-design-language-has-no-look-for)).

## 5. The shell: a bar and a dock

The panel becomes two programs (wish 1). Each is a client of the
compositor as the panel is now, started and started again in the same
way (ADR 28). The panel that is there today goes when both are done.

What both need first:

- [ ] A decision on assistive technology, as an ADR, before how the bar
      is reached is fixed
      ([finding 13](reviews/2026-11-26-accessibility-design-wcag.md#13-assistive-technology-has-no-place-in-the-model-of-roles-yet)): a role for the tools that read and drive
      the session, what it may do while locked and while the prompt is
      sealed, and its three parts in their order: programs that speak
      AT-SPI read by Orca, what omorfos draws said aloud, and tools that
      drive the pointer and the keys.
- [x] Menus and small windows that hang from a bar: a popover with
      content, placed by the compositor, closed by a click elsewhere
      (`omorfos_shell::popover`, an `xdg_popup` on the bar's layer surface,
      which takes the click that opened it); drawn as the theme's
      `popover` and `popover-item` say; resized as what is in it changes.
      The compositor keeps every popup on its screen as far as it asks,
      turned over or slid, as a bar's along the bottom is turned to hang
      above it, and as a program's menu near an edge of the screen is.
- [x] Tooltips (ADR 87): a popup of the component's surface that takes
      no grab and lets the pointer through, beside the popover that may be
      open (`Popovers::tip`), shown once the pointer has rested half a
      second, at once on the next while one is showing, and not again on
      what was clicked until the pointer leaves it
      (`omorfos_shell::tooltip`); drawn as the compositor's own over the
      capture mark is (`omorfos_ui::tooltip`). On the panel: the date over
      the time, a label cut short whole, and what a click on what says the
      session is kept awake does.
- [ ] Known gaps in tooltips: a component's casts no shadow, though the
      theme gives `tooltip` one, as its surface is only as large as its
      box; how long the pointer rests before one shows is fixed, not a
      setting; and that the pointer goes through one is not tested by
      what the pointer does.
- [x] Icons from the icon theme in use, at any size and scale: a theme's
      folders as its `index.theme` lists them (`Directories` and
      `ScaledDirectories`), each with the sizes its `Size`, `Scale`,
      `Type`, `MinSize`, `MaxSize` and `Threshold` give it, as the icon
      theme specification has them, so that Breeze's `apps/48` is found
      as well as `48x48/apps`; and what is in each folder read once, as
      the theme is chosen.
- [ ] Known gaps in icons from a theme: an icon installed while a program
      runs is seen by it only once the configuration is next read or it
      starts again; a theme's `icon-theme.cache` is not used, nor are
      `.xpm` pictures; a folder's `Context` is not looked at; and where no
      folder takes in the size, the next larger is taken rather than the
      nearest, as the specification would.

`omorfos-bar`, along the top, see-through with enough of a tint to be
read over any wallpaper. What is on it is fixed; how each thing behaves is
a setting.

- [ ] The omorfos menu, at the left, its entries from `config.yaml`. By
      default: Run, About, Settings, Task Manager, Sleep, Restart, Shut
      Down, Lock Screen, each with its key beside it. Entries whose
      program is not there yet are left out, not shown and dead.
      The maintainer chose (15 November 2026) that the bar's button
      opens the compositor's root menu, `menu.yaml`'s, hanging from the
      bar, rather than a menu of the bar's own. Done for it: the keys
      beside each row, and Lock screen, Sleep, Restart and Shut down in
      the root menu, the last two asking first unless `power.ask` is
      false (ADR 88). To come: the menu hanging from a bar it is opened
      from, and `omorfos-bar` with its button.
- [ ] The clock, by default as "Wed Apr 1 9:41 AM", its format a setting.
      A click opens a month, walked a month at a time, its weeks from
      the day the locale of the time begins them on, by its territory
      as glibc's locales have it: done in the panel
      (`omorfos_shell::month`, in English and Polish), to move to the bar
      with the clock.
- [ ] Known gaps in the month: a locale whose first day differs from
      most of its territory's (`bo_IN`, `rif_MA`, `ug_CN@latin`) is
      taken as its territory; it has no week numbers, and no events; and a theme saved while it is open closes
      it, rather than its being drawn anew as the bar is.
- [ ] The system tray (`StatusNotifierItem`), for the programs that put
      an icon there.
- [ ] The battery, if there is one: how full, and on a click how long to
      empty or to full and what state it is in (UPower).
- [ ] The network: wired or wireless and how strong; on a click the
      address, the speed, the networks in reach to choose from, and VPNs
      (NetworkManager). Joining a network needs its password asked for,
      which is the prompt of stage 3.
- [ ] Sound: the level, the output and the input, on a click.
- [ ] The notification centre: everything that has been notified since
      the session began, until it is cleared, one at a time or all at
      once. This is the "way to see what has gone" that the
      notifications lacked, and with it come buttons for a
      notification's actions and pictures that are JPEGs.

`omorfos-dock`, along the bottom.

- [ ] Programs pinned to it and programs that are running, each as its
      icon; a dot under one that is running, and a line and a tint under
      the one with focus.
- [ ] Windows of one program under one icon.
- [ ] A tooltip with the program's name; for one that is running, a
      picture of each of its windows, to pick one by, drawn afresh each
      time it is shown (see the last decision of stage 2).
- [ ] A menu on a right click: pin or unpin, close, and what the program
      itself offers in its `.desktop` file, which for a browser is "New
      Window" and "New Private Window". What a program does not publish,
      such as its open tabs, cannot be listed.
- [ ] Workspaces: which there are and which is shown, and an icon with a
      menu to go to one, make one, or bring every window to the first.
- [ ] Trash: what has been thrown away, by the freedesktop specification,
      opened in whatever manages files and emptied from a menu.
- [ ] Hidden when a window needs the room, if asked.

## 6. More of the shell

What the launcher is given to do here comes with a key of its own under
`launcher` in `config.yaml`, on by default and turned off by setting it
to false, as `launcher.sums` is (ADR 83).

- [ ] Launchpad (wish 9): every installed program on a full screen, as a
      grid or a list, in pages, arranged by dragging, with folders that
      can be named.
- [ ] A clipboard with a history (wish 9): what has been copied, to pick
      from, each thing forgotten after a time that is a setting. It
      needs a privileged protocol (`ext-data-control`), honours what
      password managers mark as secret, and keeps nothing on disk unless
      asked.
- [ ] The launcher: looser matching, as the English name of a program
      named in the user's language, and windows to go to as well as
      programs to start.
- [x] Sums in the launcher: what is typed is worked out as it is typed,
      and Enter copies the answer. Worked out by code of ours; nothing
      typed is ever run. The compositor copies for it (ADR 82), and
      `launcher.sums: false` turns it off (ADR 83).
- [x] What sums lacked: numbers read as the locale for numbers writes
      them, a comma for the decimals where it is Polish or German and
      what stands between thousands; percentages, `200+10%` being `220`
      as on a calculator; `sqrt` or `√`, and `pi` or `π`; and "Enter
      copies the answer" under the answer. Read in `omorfos-sums`, which
      the calculator is to share (ADR 86).
- [x] Sums with `sin`, `cos` and `tan` of angles in degrees, as a
      desktop's calculator has them, rounded so that what is nothing
      comes out as nothing; `ln` and `log`; and `e`.
- [ ] Known gaps in sums: angles only in degrees, with no way to ask for
      radians; a percentage in brackets, as `200+(10%)`, is a
      hundredth, not of what it is added to; which locales write a comma
      for the decimals is a table of omorfos's own, from the CLDR, and
      not what the C library says; and an answer is never written with
      anything between its thousands, which a long one would be easier
      to read with.
- [x] The launcher's text field as any text field is (ADR 85), as the
      maintainer found it wanting on 15 November 2026: the arrows, by a
      word with `Ctrl` or `Alt`, and Home and End to an end;
      Shift choosing; Backspace and Delete by a word; undo and redo; and
      copying and cutting what is chosen. One field, `omorfos_ui::Field`,
      for the other components to take up.
- [x] Pasting into the launcher, `Ctrl+V` or `Super+V`: the first line of
      what is on the clipboard, if it is text, in place of what is chosen.
      No request of its own was needed, as was thought: the clipboard is
      offered to whichever program has the keyboard, which is the launcher
      while it is open, and it pastes through Wayland's own, as any program
      does (ADR 82). Fields that ask for a secret, on the lock screen and
      in the boxes that ask for a password, take no paste, as they give
      nothing to copy.
- [x] The launcher's text field and the pointer: a click putting the
      cursor where it is, a drag from there choosing what it goes over,
      and text wider than the field shown from as far back as keeps the
      cursor in sight.
- [ ] Known gaps in the text field: a double click choosing a word; and
      text scrolled back to its start as soon as the cursor goes back
      within sight of it, rather than kept where it was, as a text field
      usually does.
- [x] Faults in the launcher found by hand on 17 November 2026:
      - its field seemed to take no more than 33 characters, where it
        takes 64 (`LONGEST`): it does take 64 of any kind, and what was
        seen is thought to be a line of one wide letter, of which 34 fill
        the field, scrolled and looking the same. Whether 64 is enough
        is for the commands and the files it is to take (below);
      - text of which a part is chosen was sometimes cut short with an
        ellipsis, as was a long line with the cursor before its end: what
        is typed is now cut at the field's edge, and what is chosen
        drawn with every letter where it stood;
      - scrolling anywhere on the screen scrolled its list, as its
        backdrop took the pointer everywhere: the backdrop takes none
        now, what is behind is pointed at and scrolled, and the
        compositor closes the launcher at a press beside it (ADR 22).
- [x] Every program as a grid (#157), as a Mac shows its Apps: the
      action `show_programs` starts the launcher with `--programs`, which
      opens onto every program installed as tiles, `launcher-tile` in the
      theme, each icon over its name, six to a line, alphabetically, and
      scrolled by lines past half the screen. Typing narrows the grid; the
      arrows walk it, Left and Right only while nothing is typed, as they
      move the cursor once something is; a sum's answer has a line to
      itself above. No key is bound to it; the four-finger pinch is to
      open it (#138). Still missing: a screen reader is told nothing of
      it, as of the rest of the launcher, which has no accessibility at
      all yet; the number of columns is fixed, not the theme's or the
      screen's; and with `launcher.icons: false` a tile keeps the room its
      icon would have.
- [x] Down gives the launcher's programs the keyboard, as Spotlight does
      (#166): while the field has it, no program is lit and Enter starts
      the first that matches; Down, or the pointer over a program, lights
      the first or that one, the field's cursor goes, and every arrow
      walks the list or the grid, until something is typed, the field is
      clicked, or Up goes back past the first line.
- [x] The cursor blinks in every text field (#167): the launcher, the
      lock screen and the box that asks for a password, by one rule in
      `omorfos-ui`, `Blink`. Half a second shown and half hidden from
      each key, and steady ten seconds after the last, so that a field
      left alone wakes nothing; never with motion reduced, or where the
      theme has the box it is in move nothing, as it shakes only then.
- [ ] Known gaps in the launcher's size: the share of the screen it takes
      at most, half, is fixed, and is to be a setting of the theme once
      it is asked for; a launcher wider than its screen is put against the
      screen's left edge, not in the middle, as Smithay makes it as wide as
      the screen and puts it there, and the launcher draws itself as wide
      as it would be anyway: it is to lay itself out at the width it is
      given. Only a screen less than 480 units wide shows it, or, for the
      grid of every program, about 700.
- [ ] The launcher's look (asked on 17 November 2026): of glass by
      default, see-through and blurring what is behind it, as menus and
      notifications are; its text field with no background of its own;
      and a sign before the field that says what the launcher is doing
      with what is typed: a magnifying glass to begin with and while it
      searches (Lucide's `search`), a calculator while it works out a
      sum (`calculator`), a terminal while it is given a command
      (`terminal`), and a grid while a program is being picked
      (`layout-grid`, or `layout-grid-circles`, as was asked for, if
      Lucide has it).
- [ ] The launcher as Spotlight, and open to more than programs (asked
      on 25 November 2026, ADR 109, #170): one launcher on `Super+Space`,
      a generic mode of sections and a top hit, and modes on `Super+1` and
      on (programs, files, actions, the clipboard's history, commands),
      each turned off or ordered in `config.yaml`; other programs' results
      through GNOME's search providers and KDE's runners. In the ADR's
      order:
      - [x] the model: the sources, the sections and the top hit, the
            modes and Super with a digit, `launcher.order`: programs and
            sums the first two sources; the generic mode a top hit and
            the programs under their heading; `Super+1` and on the modes
            built, today the programs' grid, which finds programs alone,
            `Super+0` or Escape back; the compositor hands Super with a
            digit to the launcher while it is up. What the generic mode
            shows with nothing typed, and the top hit's own heading, are
            the look's;
      - [x] the look, from the mockups of 25 November 2026 and the
            prototype of its motion: a large field, the mode's sign before
            it and its words in it; nothing under it until something is
            typed, and one dim line when nothing matches; the top hit
            outlined under its heading; the row of modes at the bottom
            while the pointer is over the empty launcher; the size gliding,
            the lit row's ground sliding, the sign turning and what the
            next mode shows fading in, and the launcher's own times in and
            out; all of it still where motion is reduced. The other modes'
            signs and words come with them;
      - [ ] actions: omorfos's, programs' own from their desktop
            entries, the user's own (`launcher.shortcuts`);
      - [ ] commands, in their mode (taking in the item below);
      - [ ] GNOME's search providers, and the files mode with them;
      - [ ] KDE's runners;
      - [ ] MPRIS, among the actions;
      - [ ] the clipboard's history;
      - [ ] the gap in `docs/STANDARDS.md`, and a proposal for
            `xdg-specs` once both kinds of provider are spoken.
- [ ] Commands in the launcher (asked on 17 November 2026): what is
      typed after `>` is run as a command in a new window of the
      terminal, by the user's shell, and the window stays open after
      it, so that what it says can be read and gone on from there. The
      `>` sets it apart from a program's name. `launcher.commands:
      false` turns it off (ADR 83). The launcher keeps nothing of what
      was run; nor does the shell's history, as the shell that runs it
      is not the one typed into. A command with nothing to say opens a
      window all the same: there is no way to run one unseen, until it
      is missed.
- [ ] The terminal, as a setting: which program it is and how it is
      given a command to run, `foot` unless said otherwise, for the
      built-in key, the menu, programs that ask for a terminal
      (`Terminal=true`) and commands in the launcher. Today each starts
      `foot`, and nothing says which the user prefers.
- [ ] Files in the launcher (asked again on 17 November 2026): those
      found by name and opened with the program for their kind. They
      are looked for in the home directory and hidden ones are passed
      over, unless the configuration says otherwise: which directories,
      and whether hidden files are among them. `launcher.files: false`
      turns it off (ADR 83). Nothing is kept on disk about them.
- [ ] Shortcuts in the launcher: a word, and what to do with the rest of
      what is typed. "google" with a question opens the default browser
      at a search for it; "ai" opens an assistant, in the browser or a
      program, with the question. Each is an entry in the configuration:
      an address with a place for the question, or a program and its
      arguments. The question is put into an address encoded and given
      to a program as one argument, never through a shell. A few come
      set up; the user adds their own.
- [ ] Hot corners and edges, if wanted.
- [ ] Widgets on the desktop, as a clock or a calendar (asked about on
      14 November 2026): first conky, whose Wayland output speaks
      layer-shell as waybar does, started from `autostart` with `may:
      [panel]` and `out_to_wayland = true`, tried on real hardware and
      written up in the guide; only if that is not enough, a clock and a
      calendar of omorfos's own, drawn with the crates and the theme the
      panel is. Not a necessity, and no API of its own: layer-shell, and
      stage 9's crates for whoever wants a widget that looks omorfos's.

## 7. Applications

Programs that come with omorfos so that it is usable on its own, and that
omorfos does not need (wish 9). Each lives outside the compositor's
workspace and is released on its own. None begins before the toolkit
decision of stage 2.

- [ ] **Task manager** (wish 8), first of these because it is what gets a
      user out of trouble. One program with two faces over one core: in
      a terminal, where it needs nothing of omorfos and so works from a
      virtual terminal when the session is hung, and in a window.
      Processes, to sort, search and end; and graphs of processor,
      memory, swap, disk, network and GPU. A key the compositor always
      answers that brings it up, even when a program has taken the
      keyboard.
- [ ] **Settings** (wish 7), with the default applications among its
      choices: the editor, the web browser, the players of video and of
      music, the viewer of pictures, the manager of files, the terminal,
      the mail program, and whatever else the standards name (the
      desktop entries' `mimeapps.list`, and the schemes such as
      `x-scheme-handler/https`). The common choices of `config.yaml` and the
      theme, written to the files as they are made and taking effect at
      once, since the files are already watched. It writes what a person
      could have written, keeps what it does not understand (for
      `config.yaml`, through `omorfos_config::edit`, ADR 81), and offers
      presets. Few options, well chosen; everything else stays in the
      files. The keyboard's settings key (`XF86Tools`) opens it.
- [ ] **Files** (wish 9), where Space shows what is chosen without
      opening it, as Quick Look does in macOS's Finder: a picture, a sound
      or a video playing, a document's pages, and text, a Markdown file
      drawn as the Markdown viewer draws it; Space again or
      Escape puts it away, and the arrows go to the next. Places and
      starred places at the side; volumes
      mounted when plugged in, and unmounted; list, details and
      thumbnails; make, rename, move, delete to the trash; archives
      opened; a quick look at a picture or a document on Space. Remote
      and cloud storage as plugins behind one interface that is designed
      at the start and filled in later. This is the largest single thing
      on this page. What is dragged out of it is shown as its icon,
      faded, with its name beside it, so that what is held is always
      said (asked on 17 November 2026): the picture of a drag is its
      program's to draw, and other file managers hand over a bare icon.
- [ ] **Image viewer** (wish 9): one picture, zoomed and turned, with
      what is known about it in a panel that is out of the way until
      asked for.
- [ ] **Screenshot**: a picture of a region drawn with the pointer, of
      the window with focus, of a window picked with the pointer, of one
      screen, or of all of them at once; copied, saved, or both. It
      stands on screen capture (stage 3) and has the `capture` role
      (ADR 37), with the compositor's mark shown as for any capture.
- [ ] **Screen recording**, in the same program, of the same five
      things, to a file. Three sounds can be recorded with it, each a
      switch and each off unless turned on: the program's, everything
      the machine plays, and the microphone. Sound comes from PipeWire.
      What is being recorded, and which sounds, is shown the whole time.
- [x] What the computer is, in "About" (stage 4 made it): the
      processor, the memory, each graphics card and internal disk, the
      system and the kernel, and the screens, read once as it opens; and
      a button that copies it as text, which programs written for X11
      can paste too (ADR 69).
- [ ] "About" opened from the bar's menu as well as the root menu, once
      the bar has one.
- [ ] Known gaps in what "About" says of the computer: a graphics card
      is named as `pci.ids` names it, which is its chip and, for most,
      the family it is sold under, not the name on its box ("Radeon
      780M"), which only the graphics driver knows; screens are named
      by their connector, as no screen's own name (EDID) is read yet;
      and Ctrl+C does not copy it.
- [x] The button of "About" that copies says it has: green, the theme's
      `about-button:active`, with a tick drawn on it and "Copied", for a
      second and a half; and About's buttons are pointed at with a hand.
- [ ] **On-screen keyboard**, for accessibility, and for a touch screen
      with no keyboard, shown and hidden by the keyboard's own key for
      it where there is one. Above every window and panel by default, and
      never taking the keyboard's focus, so that what it types goes to
      the application that has it, whatever that is, a program written
      for X11 included. It types through the compositor
      (`virtual-keyboard-unstable-v1`, or `input-method-v2` so that it
      can come up when a text field is focused), with a role of its own
      (ADR 37), since a program that may type into every other is among
      the most trusted there are. It is to be there on the lock screen
      and on the login screen too (stage 8), where there is no other way
      to type a secret without a keyboard, so it is started by those as
      well as in the session, and wears the theme in each.

More of them, asked for on 15 November 2026. Each is optional: a package
of its own, recommended by omorfos's and never needed by it, so that
leaving one out leaves nothing broken. Those marked standalone need
nothing of omorfos's session, and are packaged to install on any
desktop, as stage 8 has it for Files, the task manager and the image
viewer.

- [ ] **Calculator**, standalone: the sums the launcher works out, with
      a history of them to go back to, and a memory. The same code as the
      launcher's, `omorfos-sums`, so that a sum comes out the same in
      each (ADR 86): a function or a constant the calculator wants is
      added there, and so to the launcher too, and the launcher never
      depends on the calculator. The keyboard's
      calculator key (`XF86Calculator`) opens it where it is installed,
      and the launcher where it is not.
- [ ] **Player**, standalone: one program for a sound or a video, opened
      on a file, as QuickTime Player is on macOS, rather than a player of
      music and a player of video apart; what Files' quick look plays,
      opened whole. It plays what it is given and a queue of what was
      given with it, and keeps no library: a program that gathers music
      into albums and playlists is another thing, to be decided only if
      asked for. Decoding is the largest part and the most dangerous,
      as every file it opens is someone else's: it is done by a library
      decided with it (GStreamer, or FFmpeg), in a process of its own
      allowed nothing but to decode. Settings offers it as the player of
      music and of video, among the others installed.
- [ ] **Camera**, standalone: a picture or a video from a camera,
      mirrored as a mirror is while it is looked at, saved where the
      user's pictures and videos are. The camera through the portal and
      PipeWire, as any program asks for it, so that it works on any
      desktop, and so that the camera's being on is shown as it is for
      every program (`omorfos-recording`'s cameras).
- [ ] **Sound recorder**, standalone: the microphone recorded to a file,
      with a level to see it is heard, a list of what was recorded,
      played back in the player and renamed or thrown away. Through
      PipeWire and the portal, as the camera; the microphone's being on
      shown as for any program.
- [ ] **Markdown viewer**, standalone (asked on 15 November 2026): a
      Markdown file drawn as it reads, with its headings, lists, tables,
      quotes, code and pictures, drawn again as the file changes on disk,
      and its links opened by the program for their kind. How a Markdown
      file is read and drawn is made once, in a crate the viewer, Files'
      quick look and the text editor's preview share, so that a file looks
      the same in each, as `omorfos-sums` is shared (ADR 86). Every file it
      opens is someone else's: it is read by a parser of CommonMark that
      is widely used, `pulldown-cmark` the likely one, decided with it;
      what is HTML in it is shown as text, not run; and nothing is fetched
      from elsewhere, a picture on another machine shown as its link
      until it is clicked, so that opening a file tells no one it was
      opened. Settings offers it for `text/markdown`.
- [ ] **Text editor**, standalone, last of these: plain text, with undo
      and redo, find and replace, lines
      numbered, and a file that changed on disk said so; a Markdown file
      previewed beside it as it is typed, drawn as the Markdown viewer
      draws it. Programs as
      good exist (GNOME's Text Editor, Kate), and Settings chooses
      among them; it is built only if the toolkit decided on makes it
      small, and it never becomes an editor of code.

## 8. Reach and hardening

Ongoing, and taken between the stages above as much as after them.

Security:

- [x] The configuration, theme and IPC parsers fuzzed, beside the relay's
      reader: `configuration` and `ipc` in `fuzz/`, each run by CI as a
      job of its own on a change and for an hour each night, as
      `cargo-fuzz` needs a nightly toolchain and the gates build with the
      stable one.
- [x] Whether to read the built-in theme once, rather than at each load
      of the configuration: not. Measured, a load takes 0.72 ms, of which
      0.44 ms is the theme, mostly working out every part in every state
      rather than reading the stylesheet (85 µs); the compositor loads only
      at startup and as a file changes, and turns between day and night by
      blending themes it has. A cache would spare under half a millisecond
      an edit, and could not be bounded, as an accent may be any colour.
      The `configuration` fuzzer's thirty runs a second are the
      sanitizer's cost on that work, which a stylesheet given has to do
      anyway.
- [x] Sandboxed programs recognised and held to less
      (`security-context-v1`): done in stage 3.
- [ ] What a sandbox says its program is (the application's name) kept
      with the connection, for the portals to ask the user by, and
      carried by the relay in its `sandboxed` hand, which keeps only
      that the program is inside one (ADR 61, 3d).
- [ ] A safe way to write into a buffer a program shares, so that it
      need not be found through `/proc` (ADR 61): asked of Smithay on
      14 November 2026, as Smithay#2288 ([UPSTREAM.md](UPSTREAM.md)).
      Waits on: Smithay's answer.
- [ ] A safe way to hold a stream's buffer and to give one memory of the
      caller's own, so that `omorfos-pipewire` can go (ADR 41): offered
      to the PipeWire bindings on 15 November 2026, as pipewire-rs#124,
      in five changes, their maintainers asked how they would have the
      first shaped ([UPSTREAM.md](UPSTREAM.md)). Wanted soon. Waits on:
      their answer.
- [x] A review of everything a client can make the compositor allocate or
      wait for: done on 15 November 2026, by Mythos 5.2, and written up in
      [reviews/2026-11-15-what-a-client-can-cost.md](reviews/2026-11-15-what-a-client-can-cost.md), with how each finding was shown, what to change and the
      test to write first. What it found is below, in the order to take
      it; each is a change of its own, and none waits on another. What it
      did not look at is at its end.
- [x] The compositor waiting for ever on a pipe a program hands it
      ([finding 1](reviews/2026-11-15-what-a-client-can-cost.md#1-the-compositor-waits-for-ever-on-a-pipe-a-program-hands-it)),
      high: a program with the keyboard that asked for the compositor's
      own text on the clipboard through a pipe already full froze the
      session. What a program is handed is now written without waiting
      on it, by one type, `omorfos_ipc::handing::Handing`, for the
      compositor, the keeper of the clipboard and the screenshot tool,
      which had the same write (ADR 90): through a pipe of our own and
      `splice`, on a thread for each, sixteen at most, for half a minute
      at most.
- [x] A window said to be enormous ending the compositor
      ([finding 2](reviews/2026-11-15-what-a-client-can-cost.md#2-a-window-said-to-be-enormous-ends-the-compositor)),
      high: a titlebar as wide as a window said to be two thousand million
      units across was 224 GB to allocate, and the process was aborted.
      A frame is now at most 16384 units a side, held in the domain; a
      canvas has a most number of pixels; and what a program shows
      becomes what is drawn in one module, which leaves out a surface
      said to be larger and takes a window to be no larger (ADR 91).
- [ ] What fixing the first two showed and left open, each small and
      none a way to stop or end the session as far as was read:
      - reading a paste is written twice, in the launcher and in the
        keeper, each bounded its own way: one reader, beside `Handing`
        in `omorfos-ipc`, for both
        ([finding 1](reviews/2026-11-15-what-a-client-can-cost.md#1-the-compositor-waits-for-ever-on-a-pipe-a-program-hands-it));
      - where a surface is, which its program says as it says how large,
        was thought to be added to in whole numbers that wrap: read
        through, it is not, as Smithay's points saturate and the
        compositor takes a program's positions only through them;
        tested at the ends of the numbers;
      - every window costs a titlebar painted as wide as its frame, and
        nothing held a program to a number of windows: now a program
        has at most 256 at once, one past them never configured (ADR 91).
- [x] Any program taking the keyboard with a serial it made up
      ([finding 3](reviews/2026-11-15-what-a-client-can-cost.md#3-any-program-can-take-the-keyboard-with-a-serial-it-made-up)),
      medium: an activation token was honoured for having a serial, not
      for having one the compositor gave. Now the last key or button
      pressed has to have gone to the program that made the token, and
      its serial be that press's or one since (ADR 36); and at most 32
      tokens are kept, none longer than a minute.
- [x] Descriptors running out, in the compositor and in the relay
      ([finding 4](reviews/2026-11-15-what-a-client-can-cost.md#4-descriptors-run-out-in-the-compositor-and-in-the-relay)):
      shown worse than written, as a program connecting with none left
      ended the compositor. A socket that cannot be accepted from is now
      listened to again a moment later, in the compositor, its IPC server
      and the relay; the three programs raise their limit to the hard one
      (ADR 93); and the relay holds a program to 4096 descriptors.
- [ ] What fixing descriptors left open: the compositor holds no program
      that connects without the relay to a most number of descriptors.
      The descriptors are Smithay's, kept for pools and buffers, with no
      one place in the compositor to count them; asked of Smithay in
      `docs/UPSTREAM.md`, as what is added to an object was. In a
      session under the holder with a relay that works, every program
      comes through the relay and is held to 4096 (ADR 61).
- [x] What is added to an object kept without end
      ([finding 5](reviews/2026-11-15-what-a-client-can-cost.md#5-what-is-added-to-an-object-is-kept-without-end)),
      medium: the relay now ends a program that adds more than 4096 to
      one object. The same is to be asked of Smithay, for a compositor
      with no relay (`docs/UPSTREAM.md`).
- [x] Each kind a program offers costing the X server an atom and the
      compositor a wait
      ([finding 6](reviews/2026-11-15-what-a-client-can-cost.md#6-each-kind-a-program-offers-costs-the-x-server-an-atom-and-the-compositor-a-wait)),
      medium: at most 32 kinds, each written as a kind is, are now handed
      on to programs written for X11; the wait itself is to be asked of
      Smithay (`docs/UPSTREAM.md`).
- [x] What was refused remembered without end, by a name the program chose
      ([finding 7](reviews/2026-11-15-what-a-client-can-cost.md#7-what-was-refused-is-remembered-without-end-by-a-name-the-program-chose)),
      low to medium: at most 64 remembered now, and a name shown without
      control characters.
- [x] A program refused is told of once for each connection, not each
      name it gives itself, so that one renaming itself as it asks again
      is not told of up to 64 times (finding 7): each connection notes
      what it was refused, and the user hears of each once for it.
- [x] Every notification of idleness set again at every key and every move
      ([finding 8](reviews/2026-11-15-what-a-client-can-cost.md#8-every-notification-of-idleness-is-set-again-at-every-key-and-every-move)),
      low to medium: a program is held to 16 at once.
- [ ] Coverage that falls short on a host run at full parallelism, seen
      on 16 November 2026 in `omorfos-pipewire` and the portal, a line
      each, in runs with all 32 threads and not with 8; and in the relay's
      `link.rs`, which was a shortfall of another kind (one build's copy of
      a function not covering what another's did, `docs/TESTING.md`,
      "Covered by one build") and is fixed. The two that come with load
      are to be found: each a path reached only when threads run in a
      certain order.
- [x] Gates for a repository of many programs: a commit of documents, or
      of a program nothing else uses, ran every test before it was made
      and the coverage gate before it was pushed. Now `commit` runs the
      tests of each crate a change can reach, from Cargo's graph, and
      `commits` passes one that reaches none as the commit before it; the
      coverage gate is not narrowed, as only a run of everything shows a
      line of one crate left unreached by a change to another
      ([ADR 94](adr/0094-the-commit-tier-tests-what-a-change-can-reach.md)).
- [ ] The coverage gate narrowed soundly, once the compositor's tests are
      no longer most of a run: each test binary's profile kept from run to
      run, keyed by its build and the files it reads, and only those built
      anew run again (ADR 94, "Alternatives").
- [x] A push in one coverage run: the gate ran on every commit not yet
      pushed, five minutes each, and twice on the last where there was a
      graphics card. Now it runs once, on the last commit, as CI runs it;
      each other passes `commit`, written down as it is made; the tests
      that need a card run without the gate; and `push` runs in a
      worktree of its own, so that work goes on while it runs
      ([ADR 96](adr/0096-the-coverage-gate-is-the-last-commits.md)).
- [ ] The tests quicker: the dependencies of a test build optimised,
      as the tests that draw spend their time in tiny-skia, unoptimised;
      the three tests that run for over a minute (the panel's clock, the
      notifications that slide up, the launcher's icons) found out, as
      each turns a clock that should cost nothing; and `cargo-nextest`
      tried, which runs each test in a process of its own, so that one
      that starts a program no longer lends it every other's descriptors.
- [x] A review, for security, of what the review of 15 November did not
      look at: done on 16 November 2026, and written up in
      [reviews/2026-11-16-what-the-first-review-did-not-look-at.md](reviews/2026-11-16-what-the-first-review-did-not-look-at.md).
      What it found is below, with what the review of the whole tree
      found, in the order to take it.
- [x] A review of the whole tree in every dimension: done on 16 November
      2026, and written up in [reviews/2026-11-16-the-whole-tree.md](reviews/2026-11-16-the-whole-tree.md).
- [x] High: any program takes the keyboard from the window typed in,
      with a pop-up grabbed with a serial it made up
      ([finding 1](reviews/2026-11-16-the-whole-tree.md#1-any-program-can-take-the-keyboard-from-the-window-being-typed-in-with-a-pop-up)):
      a menu grabs only with a press that went to its program, as
      activation is asked for; any other is closed at once.
- [x] High: a program that makes a sandbox stops the relay, and every
      program, for good
      ([finding 1 of what was not looked at](reviews/2026-11-16-what-the-first-review-did-not-look-at.md#1-a-program-that-makes-a-sandbox-can-stop-the-relay-for-good)):
      each sandbox is listened on by a thread of its own. Smithay's
      listener, which a compositor without the relay still uses, is in
      `UPSTREAM.md`.
- [x] A program inside a sandbox that makes another is ended, as the
      compositor would: the relay answers for every link, and the
      compositor cannot refuse a bind it does not see in time.
- [x] High: every other program stops for half a second whenever the
      compositor sends one away
      ([finding 2](reviews/2026-11-16-the-whole-tree.md#2-whenever-the-compositor-sends-one-program-away-every-other-stops-for-half-a-second)):
      the program is kept aside for the half second, and decided on in a
      later turn.
- [x] Medium: a window's `app_id` opened as a file on the compositor's
      thread
      ([finding 3](reviews/2026-11-16-the-whole-tree.md#3-a-windows-name-for-its-program-is-opened-as-a-file-on-the-compositors-thread)):
      an `app_id` with a `/` is looked up as no icon, and a picture is
      opened without waiting and read only if it is a plain file.
- [x] Medium: a program written for X11 ends programs written for
      Wayland through the clipboard's kinds
      ([finding 4](reviews/2026-11-16-the-whole-tree.md#4-a-program-written-for-x11-can-end-every-program-written-for-wayland-that-is-given-the-keyboard)):
      the kinds are held to the same few, each written as a kind is,
      both ways.
- [x] Medium: any program on the bus puts up the portal's sealed prompt
      under any name
      ([finding 2 of what was not looked at](reviews/2026-11-16-what-the-first-review-did-not-look-at.md#2-any-program-on-the-session-bus-can-put-up-the-portals-sealed-prompt-under-any-name)):
      only the portal is answered what asks the user; sessions are held
      to 64 and forgotten when the portal starts anew; an application is
      named only by an identifier.
- [x] Medium: a program has the one that holds the clipboard let go of
      by the relay
      ([finding 3 of what was not looked at](reviews/2026-11-16-what-the-first-review-did-not-look-at.md#3-another-program-can-have-the-program-that-holds-the-clipboard-let-go-of-by-the-relay)):
      towards a program, the relay stops reading short of the most
      descriptors kept, as it does short of the most bytes.
- [x] Medium: a thread that cannot be started ends the program, the
      compositor among them
      ([finding 6](reviews/2026-11-16-the-whole-tree.md#6-a-thread-that-cannot-be-started-ends-the-program-the-compositor-among-them)):
      every thread is started by `omorfos_ipc::threads::start`, and the
      lint refuses `std::thread::spawn` outside the tests (ADR 95).
- [ ] X's resources set one change at a time: the compositor's thread
      reads, changes and writes `RESOURCE_MANAGER` without a lock, so two
      changes of scale in quick succession can leave the older, and it
      waits on the X server without bound (the rest of
      [finding 6](reviews/2026-11-16-the-whole-tree.md#6-a-thread-that-cannot-be-started-ends-the-program-the-compositor-among-them)).
- [x] Medium: the lock not waited for at sleep after one failed hold
      ([finding 8](reviews/2026-11-16-the-whole-tree.md#8-after-one-failed-hold-on-sleep-at-waking-the-lock-is-never-waited-for-again)):
      a hold that fails at waking ends the watcher, as one at the start
      does, and the compositor starts it again.
- [x] Medium: a configuration broken at a restart put aside for the
      defaults
      ([finding 7](reviews/2026-11-16-the-whole-tree.md#7-a-configuration-broken-when-the-compositor-starts-again-is-put-aside-for-the-defaults-not-the-last-that-loaded)):
      the compositor keeps a copy of each file that loads in
      `$XDG_RUNTIME_DIR/omorfos/last-good/` and starts from it (ADR 3).
- [x] The parts of the session started again (ADR 28) start from what
      the compositor keeps of the configuration too, and so do the portal,
      the clipboard's keeper and the password box; they read the copies
      and leave the keeping to the compositor.
- [ ] Medium: a buffer of the card not finished holds up drawing; with a
      card
      ([finding 4 of what was not looked at](reviews/2026-11-16-what-the-first-review-did-not-look-at.md#4-a-programs-buffer-of-the-graphics-card-that-is-not-finished-holds-up-drawing)).
- [x] Medium: on a card, an idle session wakes at the screen's rate
      ([finding 5](reviews/2026-11-16-the-whole-tree.md#5-on-a-graphics-card-a-session-with-nothing-changing-wakes-at-the-screens-rate-for-good)):
      the backend says a frame has gone, where none was drawn, only
      while something on that screen waits to be told
      (`Compositor::awaits_frame`), which looks at just what is told.
      Counted by hand on a laptop's own screen at 60 Hz, with nothing
      moving: one and two waits for something to happen in ten seconds
      (`perf stat -e syscalls:sys_enter_epoll_wait`), where there were
      600 and more.
- [x] The tests that need a card run on a virtual one where there is
      none: `OMORFOS_TEST_CARD=/dev/dri/card0` on vgem, drawn on by
      llvmpipe, which WSL2's kernel has built in (`docs/TESTING.md`). The
      portal opens the node the compositor names, primary or not.
- [x] The test of a shared window that grows and shrinks back reaches
      what lets go of a picture whose memory PipeWire takes away
      (`xdg-desktop-portal-omorfos/src/lib.rs`, `sharing.feed.abandon`,
      and what gives back a buffer still held in `omorfos-pipewire`) only
      while the portal is quick enough to have asked for the picture
      before the window changes size: in the slower build the coverage
      gate measures, a portal that did a little more as it was asked how
      the desktop looks missed it, each time. The picture is to be held
      by the test, not by timing, so that the gate cannot pass or fail by
      chance. It now waits until the compositor has the portal's request
      for a picture (`Compositor::pictures_asked`) before the window
      changes size, where it waited 200 ms. With the portal made 400 ms
      slower after each picture, the old test passed without reaching
      it; the new reaches it at each change of size.
- [ ] In CI too: a job that boots the generic kernel of Ubuntu 26.04
      under virtme-ng, as drmkit does, as the runners' Azure kernels have
      neither vgem nor vkms, with `libgl1-mesa-dri` and `libegl-mesa0`
      installed. Then `gpu.rs` is the first file to come off ADR 2's
      list; `shader.rs` and `card.rs` need their errors made reachable
      first. The hardware backend, on vkms, comes after.
- [x] Medium: the keyboard's row of a menu unseen in the light scheme
      ([finding 9](reviews/2026-11-16-the-whole-tree.md#9-in-the-light-scheme-which-row-of-a-menu-the-keyboard-is-on-cannot-be-seen)):
      the row the keyboard is on is `menu-item:focus`, the accent with
      `--on-accent`; `:hover` is the pointer's.
- [x] Medium: the screenshot's help and a program that cannot start said
      in English only
      ([finding 13](reviews/2026-11-16-the-whole-tree.md#13-two-things-the-user-is-told-come-only-in-english)):
      both are messages, and a program that is not installed is said to
      be, with what to do.
- [ ] The names of the screenshots' folder and files, and the portal's
      prompts and lines of help, in the user's language: still English,
      as the rest of finding 13 found.
- [x] Medium: the manual pages say restarting closes the windows
      ([finding 14](reviews/2026-11-16-the-whole-tree.md#14-the-manual-pages-and-the-schemas-say-that-starting-the-compositor-again-closes-the-windows)).
- [x] Medium: the test of the most windows reaches only those waiting
      ([finding 15](reviews/2026-11-16-the-whole-tree.md#15-a-program-can-map-any-number-of-windows-if-the-count-of-those-shown-were-lost-and-no-test-would-know)).
- [x] Medium: the IPC state built every turn
      ([finding 10](reviews/2026-11-16-the-whole-tree.md#10-every-turn-builds-what-is-told-to-ipc-peers-and-copies-it-though-none-listens)).
- [x] Medium: every window looked up again for the relay every turn
      ([finding 11](reviews/2026-11-16-the-whole-tree.md#11-under-the-relay-every-turn-looks-every-window-up-again-and-every-move-of-the-pointer-is-sent)).
- [x] Medium: the relay allocating for every message
      ([finding 12](reviews/2026-11-16-the-whole-tree.md#12-the-relay-allocates-for-every-message-it-passes)):
      a frame costs 9 allocations where it cost 21, and a step with
      nothing to pass one, what is polled. What is left is what is kept
      of each frame, made anew each time; the message it replaces could
      be written over instead.
- [x] Medium: putting a picture on a scaled surface written fifteen
      times
      ([finding 16](reviews/2026-11-16-the-whole-tree.md#16-putting-a-picture-on-a-scaled-surface-is-written-fifteen-times)):
      `Scale::from_numerator`, and `omorfos_shell::sheet::Sheet` for
      every surface that shows one picture at a time. The picker's
      surfaces, the portal's loupe and the wallpaper's fade are left as
      they are: they keep pictures to show again, take one off by
      attaching none, or wait for the commit of the surface under them.
      Each program still takes its scale's events itself.
- [x] Low to medium: one program takes every sandbox's place
      ([finding 5 of what was not looked at](reviews/2026-11-16-what-the-first-review-did-not-look-at.md#5-one-program-can-take-every-place-the-relay-keeps-for-sandboxes)):
      a program has at most 4 listened on, and one in a sandbox is let
      go if it makes one, as it was.
- [ ] Low to medium: words of a program's own in the notice for every
      key
      ([finding 18](reviews/2026-11-16-the-whole-tree.md#18-a-program-can-put-words-of-its-own-in-the-compositors-notices-by-asking-for-every-key)).
- [ ] Low to medium: the selector of tests not tested before a push
      ([finding 26](reviews/2026-11-16-the-whole-tree.md#26-the-selector-of-tests-is-not-tested-before-a-push)).
- [ ] Low: an X11 window that says it is another program
      ([finding 19](reviews/2026-11-16-the-whole-tree.md#19-a-window-written-for-x11-can-say-it-is-another-program-and-be-activated-in-its-place)).
- [ ] Low: the lock screen's clock after sleep
      ([finding 20](reviews/2026-11-16-the-whole-tree.md#20-the-lock-screen-shows-the-time-the-machine-went-to-sleep-for-up-to-a-minute-after-it-wakes)).
- [ ] Low: the relay takes a version not offered
      ([finding 21](reviews/2026-11-16-the-whole-tree.md#21-the-relay-takes-a-bind-at-a-version-the-compositor-does-not-offer)).
- [ ] Low: a drag below a window's least
      ([finding 22](reviews/2026-11-16-the-whole-tree.md#22-dragging-a-windows-edge-takes-it-below-what-its-program-says-is-its-least)).
- [ ] Low: the relay's records and the roles' byte
      ([finding 23](reviews/2026-11-16-the-whole-tree.md#23-what-the-relay-hands-the-compositor-is-mostly-written-only-in-the-code-and-a-roles-bit-can-be-taken-for-the-users)).
- [ ] Low: four components reach the domain
      ([finding 24](reviews/2026-11-16-the-whole-tree.md#24-the-architecture-says-no-component-reaches-the-domain-and-four-do)).
- [ ] Low: the password box drops a failed drawing
      ([finding 25](reviews/2026-11-16-the-whole-tree.md#25-the-password-box-drops-a-drawing-that-failed)).
- [ ] Low: tests that pass with less than they say
      ([finding 27](reviews/2026-11-16-the-whole-tree.md#27-tests-that-would-pass-with-less-than-they-say)).
- [ ] Low: smaller things the user meets
      ([finding 28](reviews/2026-11-16-the-whole-tree.md#28-smaller-things-the-user-meets)).
- [ ] Low: documents no longer so
      ([finding 29](reviews/2026-11-16-the-whole-tree.md#29-documents-that-say-what-is-no-longer-so)).
- [ ] Low: smaller things written more than once
      ([finding 17](reviews/2026-11-16-the-whole-tree.md#17-smaller-things-written-more-than-once)).
- [ ] Low: every window placed again each turn
      ([finding 30](reviews/2026-11-16-the-whole-tree.md#30-every-window-is-placed-in-smithays-space-again-on-every-turn)).
- [ ] Low: smaller things in the helpers and services
      ([finding 6 of what was not looked at](reviews/2026-11-16-what-the-first-review-did-not-look-at.md#6-smaller-things)).
- [x] Whether a program in a sandbox is told when the user is away at all
      (finding 8): decided on 16 November 2026, it is, as chat programs
      set their users away, but no sooner than after a minute, so that it
      cannot learn when each key is typed (ADR 37).
- [x] Smaller things the review found
      ([finding 9](reviews/2026-11-15-what-a-client-can-cost.md#9-smaller-things)), low, all fixed: the icons of applications
      looked up never forgotten; titles and identifiers as long as they
      came; a peer of the IPC socket read from for as long as it writes;
      and what a notification's picture, named by its path, has the
      decoder read, which was any file a drawing names, `/dev/zero`
      among them, and is now none.
- [ ] What the review did not look at, each a review of its own: the relay
      beyond what it keeps, the portal and the services on D-Bus, buffers
      of the graphics card, the hardware backend and what a graphics
      card's renderer does with what it is handed, and the helpers that
      check a secret.

The review of 19 November 2026, of the whole tree after the redesign of
windows, by eight reviewers at once, is written up in
[reviews/2026-11-19-after-the-redesign-of-windows.md](reviews/2026-11-19-after-the-redesign-of-windows.md).
What was fixed the same day is ticked. The rest is in the order agreed with
the maintainer that day, weighed against what else was open here: what
is unsafe first, then what glass costs, then screens remembered and
scaled (stage 3), then what closes stage 4, then stage 5; the first
release waits, there being no hurry for it.

Fixed with the review:

- [x] Medium: any program on the bus said what the bus says
      ([finding 1](reviews/2026-11-19-after-the-redesign-of-windows.md#1-any-program-on-the-bus-could-say-what-the-bus-says-and-so-become-the-portal-let-go-what-others-hold-or-end-the-portal-and-the-agent-that-asks-for-passwords)):
      the portal and the agent that asks for passwords believe only the
      bus that a name has changed hands or gone.
- [x] Medium: glass of two blurs made the screen draw for ever
      ([finding 2](reviews/2026-11-19-after-the-redesign-of-windows.md#2-a-window-that-asked-for-blur-behind-it-beside-glass-of-another-blur-made-the-screen-draw-for-ever)):
      the desktop is kept blurred once for each amount of blur.
- [x] Low: the wash under a dragged window faint twice
      ([finding 10](reviews/2026-11-19-after-the-redesign-of-windows.md#10-the-wash-that-shows-where-a-dragged-window-would-land-was-made-faint-twice)):
      made faint once. Its colour is left for the review of the defaults.
- [x] Low: two tests that failed by the order of things, and one that
      pressed beside where the button was (findings 14 and 15)
      ([finding 14](reviews/2026-11-19-after-the-redesign-of-windows.md#14-two-tests-failed-by-the-order-of-things-not-by-the-code)).

First, what is unsafe or gives away what is private:

- [x] Medium: a program that copies in very many kinds ends each program
      next given the keyboard, and the keeper of the clipboard
      ([finding 3](reviews/2026-11-19-after-the-redesign-of-windows.md#3-a-program-that-copies-in-very-many-kinds-ends-each-program-next-given-the-keyboard-and-the-keeper-of-the-clipboard)):
      the relay ends a program that offers more than 256 kinds from one
      source, or one longer than 255 bytes. Without the relay the
      compositor takes what is offered so off the clipboard before
      another program has the keyboard, kinds named after it was copied
      among them, which a test showed get past a count made as it is;
      the keeper of the clipboard, and a program dragged over, wait on
      Smithay there (`UPSTREAM.md`).
- [x] Medium: a portal started again leaves every screen it was sharing
      still shared
      ([finding 5](reviews/2026-11-19-after-the-redesign-of-windows.md#5-a-portal-started-again-leaves-every-screen-it-was-sharing-still-shared-with-nothing-left-that-can-end-it)):
      when the portal's name changes hands or goes, the backend is told,
      and takes down the question or picker the one before put up, as
      withdrawn, and ends every share of its sessions.
- [x] High, as shown: a private window to be blurred shows sharp
      what it draws outside its frame
      ([finding 12](reviews/2026-11-19-after-the-redesign-of-windows.md#12-a-private-window-that-is-to-be-blurred-shows-sharp-in-pictures-of-the-screen-whatever-it-draws-outside-its-frame)):
      a part of its own beyond its frame was in pictures sharp; in a
      picture, what it draws is cut to its frame where the frame does not
      cut it already, as the frame lands once stretched to a new size. A
      review of the fix found three more ways past the glass, each shown
      and closed: the picture it had before, fading after a new size, is
      left out, being noted as the screen has it; the glass grows with the
      window as it comes in, should the theme have it come in larger; and
      the glass over its picture made small reaches a pixel further, to
      the last that picture is drawn to. Stage 3's known gaps in a blurred
      private window remain.
- [x] Medium: the relay's limits hold a connection, not a program, and
      connections are not counted
      ([finding 4](reviews/2026-11-19-after-the-redesign-of-windows.md#4-what-the-relay-holds-a-program-to-it-holds-each-connection-to-and-connections-are-not-counted)):
      with "one program takes every sandbox's place" above, as one change.
      A program, known by its process, or by its sandbox if it is in one,
      is let in on at most 32 connections, and held to 4096 descriptors
      over all of them; what is added to objects stays each
      connection's, so bounded 32 times over (ADR 93). Left: a program
      outside a sandbox that starts processes is held to as much in
      each, and whether an honest program's descriptor is lost while the
      relay has none left is yet to be shown.
- [x] Low: part of a notice in English in the code
      ([finding 21](reviews/2026-11-19-after-the-redesign-of-windows.md#21-part-of-a-notice-the-user-reads-is-english-written-in-the-code)): why glass is solid, and a keyboard layout that cannot be had,
      are said in messages.
- [x] Low: "About"'s one button not pressed from the keyboard
      ([finding 22](reviews/2026-11-19-after-the-redesign-of-windows.md#22-about-has-the-keyboard-and-its-one-button-cannot-be-pressed-with-it)): Enter presses Copy, and the guide says so.
- [x] Low: a titlebar over a fullscreen window said to be square
      ([finding 23](reviews/2026-11-19-after-the-redesign-of-windows.md#23-a-titlebar-over-a-fullscreen-window-is-said-to-be-square-and-only-its-bottom-is)): it is square at the top as at the bottom, its glass with it.

Then what glass costs. Measured first, as stage 4's item says, with a
build that has the fix to finding 2, and each of these taken as the
numbers say:

- [x] Medium: glass made and blurred again on every drawing while a screen
      with a private window on it is recorded
      ([finding 6](reviews/2026-11-19-after-the-redesign-of-windows.md#6-while-a-screen-is-recorded-with-a-private-window-on-it-glass-is-made-and-blurred-again-on-every-drawing)):
      a picture of the screen keeps its glass apart from the screen's
      while a private window is shown, and shares it, blurring once for
      both, while none is. Left: a private window seen only in the
      overview's pictures of another workspace does not count, and the
      glass there is blurred for each while it is recorded.
- [x] Low, medium if measured so: every sheet of glass looks at everything
      under it on every drawing
      ([finding 13](reviews/2026-11-19-after-the-redesign-of-windows.md#13-every-sheet-of-glass-looks-at-everything-under-it-on-every-drawing-changed-or-not)):
      measured at 0.7 ms a frame with twelve covered sheets on a UHD 620
      (ADR 33). Each sheet keeps what was under it, element by element,
      and its tracker is not run, nor its framebuffer made, until that
      changes; what glass adds to a quiet turn grows no faster than the
      windows.
- [x] Medium: a sheet of glass with a window behind it has every texture
      made anew on each frame its size changes
      ([finding 7](reviews/2026-11-19-after-the-redesign-of-windows.md#7-a-sheet-of-glass-with-a-window-behind-it-has-every-texture-made-anew-on-each-frame-its-size-changes)):
      measured as more than a third of frames on a drag, and no time of
      a frame's (ADR 33). What is drawn behind glass is as large as it
      needs rounded up to steps of 64 pixels, and moved within the screen
      rather than cut by it, so a menu coming in makes its picture once
      in ten frames where it made eight, and a titlebar dragged along the
      screen's edge keeps its own.
- [ ] Low: what is written more than once in the compositor's drawing,
      which is the same code
      ([finding 26](reviews/2026-11-19-after-the-redesign-of-windows.md#26-what-is-written-more-than-once-in-the-compositors-drawing)):
      what of it is not drawing goes with stage 9's "made once". An area
      turned into pixels by hand is `View::pixels` in each place now; the
      rest is left.

Then, after screens remembered and scaled (stage 3), with what is left
of the redesign of windows and the review of the defaults (stage 4):

- [ ] Medium: the welcome card, the bar's popovers and a component's
      tooltips see-through with nothing blurred behind them, and
      `reduce_transparency` not making them solid
      ([finding 8](reviews/2026-11-19-after-the-redesign-of-windows.md#8-the-welcome-card-the-bars-popovers-and-a-components-tooltips-are-see-through-with-nothing-blurred-behind-them-and-reduce_transparency-does-not-make-them-solid)).
- [ ] Medium: a titlebar's buttons barely seen in the light scheme, and
      without focus in either scheme
      ([finding 9](reviews/2026-11-19-after-the-redesign-of-windows.md#9-in-the-light-scheme-two-of-a-titlebars-three-buttons-can-barely-be-seen-and-without-focus-none-can-in-either-scheme)):
      whether by a stronger rim or deeper fills is decided with the
      defaults, as is whether `--glass-thin` stays.
- [x] Low: who draws a titlebar said by one layer and done by another
      ([finding 11](reviews/2026-11-19-after-the-redesign-of-windows.md#11-a-program-is-told-who-draws-its-titlebar-by-one-layer-and-framed-or-not-by-another-under-a-rule-or-when-it-asks-late-the-two-disagree)):
      what a program is told is decided once, as the desktop has it: by a
      rule before its window is shown, and by its frame once it is.

Known gaps, each taken when the code beside it is touched:

- [x] Low: four more tests that say less than their names
      ([finding 16](reviews/2026-11-19-after-the-redesign-of-windows.md#16-four-more-tests-say-less-than-their-names)):
      the hairline is compared by channel with no shadow under it; the
      question after a withdrawn picture is asked until it goes up; what
      the service says is waited for ten seconds at most, and every
      setting counted from the list of them; and the question refused
      while another is up is joined, and seen to have failed for that.
- [x] Low: the portal's backend can wait for ever as it starts
      ([finding 17](reviews/2026-11-19-after-the-redesign-of-windows.md#17-the-portals-backend-can-wait-for-ever-as-it-starts-holding-its-name-on-a-busy-bus)):
      the bus may hold a thousand messages for the portal before it reads
      them, and what it asks as it starts is waited for ten seconds at
      most, so that the worst is a start that fails and is made again.
- [ ] Low: what a starting portal says may not reach programs when it
      takes its name from a copy still running; to be seen with a real
      toolkit, which no test can show
      ([finding 18](reviews/2026-11-19-after-the-redesign-of-windows.md#18-what-a-starting-portal-says-of-every-setting-may-not-reach-programs-when-it-takes-its-name-from-a-copy-still-running)).
- [x] Low: "what last loaded" read from the disk a second time
      ([finding 19](reviews/2026-11-19-after-the-redesign-of-windows.md#19-the-copy-kept-as-what-last-loaded-is-read-from-the-disk-a-second-time)):
      what is kept is the text that was parsed, not the file read again.
- [x] Low: a sandbox whose socket is not one
      ([finding 20](reviews/2026-11-19-after-the-redesign-of-windows.md#20-a-sandbox-whose-socket-is-not-one-has-every-program-let-in-late-and-the-log-say-descriptors-ran-out)):
      its thread listens no more on an error that is not one of running
      out, and the relay is neither paused nor told of it.
- [x] Low: the side and order of a titlebar's buttons said three times
      ([finding 24](reviews/2026-11-19-after-the-redesign-of-windows.md#24-the-side-and-order-of-a-titlebars-buttons-are-said-three-times)):
      the buttons are drawn in the domain's order, `Button::ALL`, and a test
      holds what programs are told to it.
- [ ] Low: the gates' quick checks listed four times, and `uncovered`
      blind to failed tests
      ([finding 25](reviews/2026-11-19-after-the-redesign-of-windows.md#25-the-gates-quick-checks-are-listed-four-times-and-have-drifted-uncovered-by-itself-cannot-see-that-tests-failed)).
- [ ] Low: documents and comments no longer so, what was more than a line
      ([finding 27](reviews/2026-11-19-after-the-redesign-of-windows.md#27-documents-and-comments-that-said-what-is-no-longer-so)):
      ACCESSIBILITY.md's four passages, `Part::ButtonClose`'s doc, the
      `all` tier in TESTING.md, ADR 94 on `scripts` and ADR 99 on how an
      entry is checked are put right. Left: `--glass-thin` and the bar's
      dim text, to decide; DESIGN.md's "Following the sun" and "over any
      wallpaper"; and TESTING.md's times, to be measured where the gates
      are run.

The review of pull requests 44 to 48, of 20 November 2026, found in
`main`:

- [x] High: any program written for X11 ends the compositor at will
      ([finding 1](reviews/2026-11-20-pull-requests-44-to-48.md#1-any-program-written-for-x11-can-end-the-compositor-at-will)):
      it closes the window manager's connection (`KillClient`), and the
      next commit of a surface the X server named asks for a manager that
      is gone. The manager let go of is kept, and Smithay is answered by
      its id; an X server that loses its manager is ended, and started
      again as one that stops is.
- [ ] To be looked at: any program written for X11 may freeze the
      compositor with `GrabServer`, as asking which screen is primary
      did; Xwayland listens on the abstract socket with no `-auth`, so a
      sandbox with the network may reach every program written for X11
      ([seen in passing](reviews/2026-11-20-pull-requests-44-to-48.md#seen-in-passing-for-later)).
- [x] Low: the welcome card says `Super+C` copies whatever
      `keyboard.super_as_ctrl` is; `omorfos-ask` takes an empty `LC_ALL`
      as set and never reads `LC_MESSAGES`
      ([seen in passing](reviews/2026-11-20-pull-requests-44-to-48.md#seen-in-passing-for-later)):
      the card says it only where Super is Ctrl, and `omorfos-ask` takes
      the first of `LC_ALL`, `LC_MESSAGES` and `LANG` set to something.

The review of 22 November 2026, of the whole tree again, by eight
reviewers at once, is written up in
[reviews/2026-11-22-the-whole-tree-again.md](reviews/2026-11-22-the-whole-tree-again.md),
in the order to take it; what was written but could not be built there
was a pull request of its own, built and tested before it was merged:

- [x] High: one connection has the relay, and the compositor behind it,
      keep about a thousand million rectangles or kinds, the bound on
      each object multiplied by the objects a link may hold
      ([finding 1](reviews/2026-11-22-the-whole-tree-again.md#1-one-connection-can-have-the-relay-and-the-compositor-behind-it-keep-about-a-thousand-million-rectangles-or-kinds)):
      a connection may keep 65,536 additions over all its objects, and
      what is added to an object let go of no longer counts (#68). Used
      on the maintainer's laptop from 22 November 2026 with nothing let
      go for it.
- [x] Medium: a configuration directory taken away and put back, or a
      link to one pointed elsewhere, never watched again by any program
      ([finding 2](reviews/2026-11-22-the-whole-tree-again.md#2-a-configuration-directory-taken-away-and-put-back-or-a-link-to-one-pointed-elsewhere-is-never-watched-again-by-any-program-of-the-session)): the directory it is in is watched for its name, and it is watched again wherever its path leads (#69).
- [ ] Medium: a window's frame decided again by each compositor, so a
      tab can come back with two titlebars; the place text's version,
      decided on 24 November 2026: what is added is read by a compositor
      before it as today, which knows nothing of the frame and does as
      it does now, since an older build is installed often to try one
      ([finding 3](reviews/2026-11-22-the-whole-tree-again.md#3-a-windows-frame-is-decided-again-by-each-compositor-so-a-framed-window-can-come-back-without-one-and-a-tab-with-two-titlebars)).
- [x] Low: the test of the question after a withdrawn picture gives up
      after about 350 ms, which the tick of finding 16 of 19 November
      does not say ([finding 4](reviews/2026-11-22-the-whole-tree-again.md#4-the-test-said-to-wait-however-long-that-takes-after-a-withdrawn-picture-gives-up-after-about-350-ms)): it asks until the question goes up, for as long as ten seconds (#76).
- [x] Low: "About" takes Super with Return, space, Escape and the arrows
      from their bindings ([finding 5](reviews/2026-11-22-the-whole-tree-again.md#5-while-about-is-shown-super-with-return-space-escape-or-an-arrow-does-what-about-does-not-what-the-key-is-bound-to)): "About" takes a key only with nothing held (#70).
- [ ] Low: without Xwayland, the notice says "os error 2" and not what is
      missing ([finding 6](reviews/2026-11-22-the-whole-tree-again.md#6-without-xwayland-installed-every-logins-notice-says-no-such-file-or-directory-os-error-2)).
- [x] Low: today faint among the days of another month ([finding 7](reviews/2026-11-22-the-whole-tree-again.md#7-today-is-drawn-as-faintly-as-the-days-of-another-month-when-the-month-shown-is-not-todays)):
      `popover-day-outside:active` has the accent too (#71).
- [x] Low: a screen's global let go of only when something else wakes the
      compositor ([finding 8](reviews/2026-11-22-the-whole-tree-again.md#8-a-screens-global-kept-five-seconds-after-the-screen-goes-is-let-go-of-only-when-something-else-wakes-the-compositor)): the time it goes is among what the compositor waits for (#72).
- [x] Low: the locale looked up by five copies of one lookup ([finding 9](reviews/2026-11-22-the-whole-tree-again.md#9-the-users-locale-is-looked-up-by-five-copies-of-one-lookup-and-the-fix-of-21-november-went-into-one)): `omorfos_i18n::locale` (#75).
- [x] Low: the limits on what a copy is offered as said in four places
      ([finding 10](reviews/2026-11-22-the-whole-tree-again.md#10-how-many-kinds-a-copy-may-come-in-and-how-long-each-may-be-is-said-in-four-places-and-nothing-holds-the-relays-and-the-compositors-alike)): `omorfos_ipc::copied` (#74).
- [ ] Low: a screen's name copied each frame, and each turn the overview,
      the switcher or the sealed prompt is up ([finding 11](reviews/2026-11-22-the-whole-tree-again.md#11-each-frame-drawn-copies-the-screens-name-and-so-does-each-turn-while-the-overview-the-switcher-or-the-sealed-prompt-is-up)).
- [x] Low: every start of the X server reads `/proc` ([finding 12](reviews/2026-11-22-the-whole-tree-again.md#12-every-start-of-the-x-server-reads-proc-and-looks-at-tmpx11-unix-though-only-a-start-that-failed-needs-them)):
      only a start that failed (#73).
- [x] Low: the compositor's count of kinds held by no test ([finding 13](reviews/2026-11-22-the-whole-tree-again.md#13-a-program-that-offers-too-many-short-kinds-is-never-shown-to-be-refused-by-the-compositor)): a copy in 2000 short kinds is refused (#77).
- [ ] Low: three fixes since 19 November run by the tests and held by none
      ([finding 14](reviews/2026-11-22-the-whole-tree-again.md#14-three-fixes-since-19-november-are-run-by-the-tests-and-held-by-none)):
      the decoration rule for a program that names itself late, the
      keypad's Enter in "About" and the relay's socket that is not one
      are held now (#78). Left: whether the manager of an X server is
      kept once its connection is cut (`lost`) needs its test, and
      `super_as_ctrl` on the welcome card is in a part no test runs.
- [x] Low: four documents no longer so ([finding 15](reviews/2026-11-22-the-whole-tree-again.md#15-four-documents-said-what-is-no-longer-so)): the roadmap's items asked
      of Smithay and the PipeWire bindings, ADR 11, `OutputScale::Auto`'s
      comment and the README's launcher put right.

Accessibility:

- [ ] Everything in the shell reachable from the keyboard.
- [ ] High contrast, larger text and reduced motion, as settings the
      portal also tells applications of.
- [ ] A magnifier: the screen, or the part round the pointer, made larger,
      following the pointer, the focus and where text is typed, by keys
      that zoom in and out and turn it on and off. The compositor's,
      since only it has the whole screen and the pointer; so it is part
      of omorfos and not a program to install elsewhere, where each
      desktop has its own.
- [ ] Sticky and slow keys.
- [ ] What a screen reader needs of the shell (AT-SPI), which depends on
      the toolkit decision. The narrator is Orca, which reads every
      desktop on Linux and every program that speaks AT-SPI, rather than
      one of omorfos's own: omorfos's part is that what it draws is told
      to it, a key that starts and stops it (`Super+Alt+S`, as GNOME
      has), and the packages recommending it.

The review of 26 November 2026 (stage 4 has where it is written up, and
its findings of the look) found these besides, in the order to take
them. The first two are of security, and are written down in the open as
the maintainer decided that day, omorfos having had no release: they are
to be put right before a release of any kind, and the first release
(below, under "Reaching people") waits for them:

- [x] **High: a key meant for another program says yes to sharing the
      screen**
      ([finding 3](reviews/2026-11-26-accessibility-design-wcag.md#3-a-key-meant-for-another-program-says-yes-to-sharing-the-screen)):
      the portal's question took the keyboard as it came up, and Enter
      was "Share". Every box that asks is answered now as ADR 111 has
      it: the keyboard starts on what refuses, shown; what gives waits
      half a second after the box is up; a key counts as it is let go,
      having gone down while it was up, and a press as it is let go on
      what it went down on.
- [x] **High: a box for a secret that is withdrawn hands the keyboard
      straight back**
      ([finding 26](reviews/2026-11-26-accessibility-design-wcag.md#26-a-box-for-a-secret-that-is-withdrawn-hands-the-keyboard-straight-back)):
      the rest of a password was typed into the window under it. A prompt
      that goes unanswered now leaves the keyboard to nothing until every
      key is up and half a second has gone by with none pressed, or until
      the pointer is pressed (ADR 48, addendum).
- [ ] Medium: the boxes that ask not answering alike
      ([finding 14](reviews/2026-11-26-accessibility-design-wcag.md#14-the-boxes-that-ask-do-not-answer-alike)).
      The portal's question, the one before leaving, the screens' and
      the password box's buttons answer alike since ADR 111; what is
      left is the rest of the finding.
- [ ] Medium: nearly everything acting as the button goes down
      ([finding 5](reviews/2026-11-26-accessibility-design-wcag.md#5-nearly-everything-acts-as-the-button-goes-down)).
      The boxes that ask act as it is let go since ADR 111; menus, the
      launcher, the bar and notifications do not yet.
- [x] Medium: times that cannot be changed, and the question before leaving
      saying yes by itself after a minute
      ([finding 7](reviews/2026-11-26-accessibility-design-wcag.md#7-times-that-cannot-be-changed-and-one-that-says-yes-by-itself)).
      ADR 112: `power.wait`, put off by being there; the screens'
      question with buttons, put off to two minutes at most and put back
      at once by Escape; a notification held while pointed at, the user's
      timeout the shortest; `mouse.double_click`; the lock screen's
      pointer. Left: the volume shown for a second and a half, a
      tooltip's half second, a full screen's bar.
- [ ] A history of notifications, to read again one that has gone: what
      finding 7 found missing, a feature of its own (ADR 112).
- [ ] Medium: which program is taking pictures told only to a pointer at
      rest
      ([finding 6](reviews/2026-11-26-accessibility-design-wcag.md#6-which-program-is-taking-pictures-is-told-only-to-a-pointer-at-rest)).
- [ ] Medium: a window moved and resized only by dragging, and what else
      only a drag or a pointer does
      ([finding 4](reviews/2026-11-26-accessibility-design-wcag.md#4-a-window-is-moved-and-resized-only-by-dragging)): its table is "everything in the shell
      reachable from the keyboard", above, a row at a time.
- [ ] Low: chords of four and five keys, and things with no key or no place
      to press
      ([finding 21](reviews/2026-11-26-accessibility-design-wcag.md#21-chords-of-four-and-five-keys-and-things-with-no-key-or-no-place-to-press)).
- [ ] Medium: accessibility settings turned on only by editing a file
      ([finding 12](reviews/2026-11-26-accessibility-design-wcag.md#12-accessibility-settings-are-turned-on-only-by-editing-a-file)).
- [ ] Medium: a passphrase that cannot be pasted, nor anything looked up
      while it is asked
      ([finding 8](reviews/2026-11-26-accessibility-design-wcag.md#8-a-passphrase-cannot-be-pasted-nor-anything-looked-up-while-it-is-asked)).
- [ ] Medium: the lock screen not saying why a password failed
      ([finding 9](reviews/2026-11-26-accessibility-design-wcag.md#9-the-lock-screen-does-not-say-why-a-password-failed)).
- [ ] Medium: larger text cut, and a larger scale running off the screen
      ([finding 10](reviews/2026-11-26-accessibility-design-wcag.md#10-larger-text-is-cut-and-a-larger-scale-runs-off-the-screen)).
- [ ] Low: four things smaller than 24 by 24
      ([finding 18](reviews/2026-11-26-accessibility-design-wcag.md#18-four-things-are-smaller-than-24-by-24)).
- [ ] Low: reduced motion leaving two things moving
      ([finding 19](reviews/2026-11-26-accessibility-design-wcag.md#19-reduced-motion-leaves-two-things-moving)).
- [ ] Low: what Enter starts in the launcher following a pointer that did
      not move
      ([finding 20](reviews/2026-11-26-accessibility-design-wcag.md#20-what-enter-starts-follows-a-pointer-that-did-not-move)).
- [ ] Low: words that are the house's own, and two spellings
      ([finding 23](reviews/2026-11-26-accessibility-design-wcag.md#23-words)).
- [ ] Low: fifteen things ACCESSIBILITY.md does not list as not there yet
      ([finding 24](reviews/2026-11-26-accessibility-design-wcag.md#24-what-is-listed-as-not-there-yet-leaves-out)).
- [ ] Low: documents that say more than is so
      ([finding 25](reviews/2026-11-26-accessibility-design-wcag.md#25-documents-that-say-more-than-is-so)).
- [ ] The session walked through by hand, which the review could not
      do: the keyboard alone from logging in to logging out; Orca with
      one program of GTK and one of Qt; a screen of 1366 by 768 at a
      scale of 2; and the theme at 22pt.
- [ ] Accessibility as a dimension of its own in the review routine,
      where it is one word among those of "experience".

Hardware and performance:

- [ ] A window that fills the screen shown without being copied (direct
      scanout), and the refresh rate following it (VRR).
- [ ] Touch screens and drawing tablets.
- [ ] Wide colour and HDR: a screen that can show more than sRGB, or
      brighter than white, driven so, and a program that draws for it
      shown as it drew. What it takes:
      - programs told what each screen can show, and saying what colours
        and brightness they drew in, through `wp_color_management_v1`,
        whose description `wayland-protocols` already carries (staging);
      - the screen told, through its connector's `Colorspace` and
        `HDR_OUTPUT_METADATA`, that it is sent HDR (PQ, BT.2020), and
        driven at 10 bits or more;
      - the session composed in a space wide and deep enough for both,
        what was drawn for sRGB, the panel, titlebars and most programs,
        brought into it at a brightness of the user's choosing, and what
        is brighter than the screen can show tone-mapped to fit;
      - pictures of the screen and sharing it, which programs expect in
        sRGB, mapped back down, and the colour picker reading what was
        drawn, not what was sent;
      - a setting for it, each screen's own, off unless the screen says
        it can and the user says so.
      Waits on Smithay, which has no handler for the protocol, composes
      in 8-bit sRGB, and sets neither connector property. Night light
      (stage 4) warms a screen through its gamma table, which an HDR
      screen needs for its own curve: there it is to warm the picture as
      it is composed instead.
- [ ] Measured: what an idle session costs, and how long a frame takes.
- [x] The third priority as a gate: `quiet::` holds that a session with
      nothing changing, a window open, draws nothing, has nothing due to
      wake it, and keeps nothing it allocates, turn after turn; and that
      a turn allocates no more than it does today, 42 times then.
- [x] `sync` allocating nothing for a window with no frame of ours on a
      turn in which no window came or went: what it builds in is kept
      from turn to turn, the screens' names are kept rather than copied
      out of Smithay to work out which comes first, and nothing is put
      aside for titles when there are none.
- [x] What programs written for X11 are told worked out only when the
      configuration or the scale has changed, not on every turn to find
      it the same; a program written for X11 costs a turn with nothing
      changing no more than any other.
- [x] A frame's titlebar known again at no cost when it looks as it did:
      asked of what it was painted as, part by part, rather than of a new
      copy of all of it, its tabs' looks worked out in the room they had,
      and its shadow not placed again where it is.
- [x] Whether a screen is given over to windows filling it asked of the
      domain without listing every workspace (`Desktop::shows_tiles`), the
      dims of the sealed prompt kept up to date without a list of the
      screens, and a screen's name borrowed, not copied, to draw its glass.
- [x] The strips a window is cut into for its frame's rounded corners
      kept with the frame, and worked out again only when the frame, the
      window or the radius changes, rather than twice a turn.
- [ ] Every program that loads the configuration woken by whatever is
      made, taken away or renamed beside its directory, as in `~/.config`
      as each program there saves a file by renaming: since the watcher
      watches the directory it is in for it being replaced (ADR 3), to do
      nothing. Found on 23 November 2026, when it made the two tests that
      count the loop's turns while no descriptor is left fail on a busy
      `/tmp`, where each test's configuration was then; it is now in a
      directory of its own. inotify cannot watch one name in a directory
      alone, so the cost is to be weighed against what the watch catches,
      or the watch made on the directory itself only, with what is lost
      where it is replaced said in the guide.
- [ ] A turn with nothing changing allocating nothing, the gate's ceiling
      lowered as each goes. With one window in a frame of ours, as `quiet::`
      has it since the X11 work showed the frame was being left out, 13:
      10 in drawing, where what is drawn is made anew each time, as lists
      of elements, much of it Smithay's for what the program drew, which
      keeping them from frame to frame would take a rework of drawing and
      part of it Smithay's; and 3 in the turn before it, Smithay's
      refreshing of the space among them, which waits on Smithay.
- [ ] What is claimed of speed made by a script, and each number kept
      with the commit, the card, its driver and how many runs it is of.
- [ ] A window on another desktop (the nested backend) that does no work
      when nothing changes. It draws again at each frame of the host's,
      as the swap paces it, and swaps a frame where nothing changed: 7%
      of a core under WSLg with nothing moving. The hardware backend's
      way, a frame said to have gone only while something waits for one,
      would do, with what the window is told of the host's frames.

Reports and recovery:

- [x] `omorfosctl report`: the versions, the graphics cards, the
      configuration, the end of the session's logs, which the holder now
      keeps, this login's and the one before's, in
      `~/.local/state/omorfos` (ADR 60), and the state the IPC socket
      gives, gathered into one text for the user to look over before
      handing it on. Window titles and paths in the home are left out
      unless `--whole` is given, and nothing is sent anywhere. Made
      without a compositor too.
- [x] What the report lacked: a path elsewhere that names the user, as
      a mount under `/media`, left out after the parts before it; the
      journal's last thousand lines of this login, or of the user since
      the computer started where the login is not known; and a login's
      log kept within 8 MiB by the holder, cut to its last 2 MiB (ADR 60).
- [x] A user whose name is also a common part of the system's paths, as
      `share` or `bin`, named in a report only where the user's own disks
      are mounted, under `/media`, `/run/media` or `/mnt`, and not in
      every `/usr/share`.
- [ ] Known gaps in the report: the journal's lines of this login are all its programs', not
      only omorfos's; and a line written in the moment the log is cut is
      lost, or put before what was kept.
- [ ] The report does not know its login where the program it is run
      from does not, found by hand on 17 November 2026: a terminal that
      the user's systemd starts, as ghostty is, has no `XDG_SESSION_ID`,
      so the report run in it gives "The journal, the user's since the
      computer started" in place of this login's. It is to ask the
      compositor, or logind, which login it is of.
- [ ] The user's name by itself, and the computer's, left out of a
      report unless `--whole` is given (decided on 17 November 2026).
      Only paths are left out today, so the journal's lines of `sudo`,
      `pkexec` and PAM name the user, and every line of the journal
      names the computer.
- [x] What `gpg-agent` gives the pinentry to say kept out of the
      session's log, and so out of a report (seen by the maintainer on
      22 November 2026): for a smartcard it names whose card it is and
      its number, and for a key its owner's name and address. It was
      passed to `omorfos-ask` for the log alone, and is passed no more.
- [ ] The screen kept lit while the compositor is started again (ADR
      60): the holder holding the screen's device, so that the last frame
      stays until the next compositor draws over it. The hardware
      backend's, which no test runs.

Programs written for X11:

- [ ] The X server kept when the compositor is started again, and the
      windows written for X11 with it (ADR 60), an item of its own as
      the maintainer decided on 24 November 2026 (#124): the X server is
      the compositor's today and goes with it, as the gap below says, so
      every program written for X11 goes, and every program on Wayland
      that holds a connection to it, as Firefox ESR does wherever
      `DISPLAY` is set. To keep it, it would be held as the socket of
      Wayland is, and taken up by each compositor as its window manager;
      but Smithay's window manager takes up only windows made after it
      starts. Waits on: Smithay. Weighed against the rest before it is
      begun.
- [ ] Known gap in what goes with the X server, found on 18 November
      2026: the X server is the compositor's and is started anew with
      each (ADR 61 has a program written for X11 close with it, and a
      new version wait for such a program's windows to be closed). A
      program that only holds a connection to it goes when it does,
      though its windows are Wayland's and the relay keeps them. Visual
      Studio Code is one, and says "X connection to :0 broken" as it
      goes. Such a program has no window written for X11, so the wait
      for a quiet moment before a new version is taken up does not see
      it. To count what is connected to the X server, not its windows
      alone: the X server says who is connected, and whose process each
      is, through its X-Resource extension, but the compositor cannot
      ask and wait for the answer, as the server may be waiting on the
      compositor at the time (as asking which screen is primary froze,
      above). So it is to be asked from a thread of its own, as any
      program would ask, while a new version waits, and what it last
      said is what the wait goes by.
- [ ] Known gap, found on 20 November 2026 and not yet seen happen: a
      paste from a program written for X11 into one written for Wayland
      that is under way as its X server ends. Smithay leaves the
      transfer's pipe in the event loop when the window manager is
      dropped, held open while its source waits, so the Wayland program
      reading it waits for an end that does not come; and cannot be asked
      whether a transfer is under way, so the compositor cannot tell when
      a manager it let go of may go. What once made this end the
      compositor, `xwm_state` asked for a manager that was gone, is the
      high finding of the review of pull requests 44 to 48. To do: once
      Smithay closes a dropped transfer's pipe, or says whether one is
      under way, drop the manager let go of when nothing of it is left,
      and have a test end an X server in the middle of a paste. What
      Smithay leaves in the loop is in `docs/UPSTREAM.md`.
- [ ] Dialogs kept with the window they belong to, and docks, splash
      screens and the like treated as such.
- [ ] The X server started when first asked for. Waits on: a change in
      Smithay, which is worth offering to it, as is the fix for what it
      leaves unsent (ADR 27).

Known gaps carried forward:

- [x] During a resize from the left or top edge, the window's content can
      lag the frame by a frame or two, until a window that changes size
      does so smoothly (stage 4, "Movement"): its last picture is now
      stretched to the frame until it has drawn, where windows change
      over a time (ADR 72).
- [x] Dragging and dropping between applications, which works through
      Smithay's defaults, tested through two real clients (`drag_and_drop::`):
      text dragged from one and dropped on the other arrives, and both are
      told the drop was taken; one let go where nothing takes it is given
      back.
- [x] The picture a program drags something with drawn under the pointer
      while it drags, where the program has it hang, and gone once it is
      let go: Smithay hands it over and leaves the drawing to the
      compositor, which drew nothing.
- [x] A picture dragged that moves told when to draw its next, as the
      pointer's is; and what is dropped to be moved, rather than copied,
      told to its program as moved, which a file manager lets go of what
      it moved by, tested.
- [ ] Known gaps in dragging and dropping: no test drags between a
      program written for X11 and one written for Wayland.
- [x] Two tests that failed only sometimes, both mended on 20 October
      2026 and found so on 15 November: what a program written for X11
      asks of a window not on the desktop is now waited for until it is
      heard (`a066bea`), and the region it reaches was covered in each of
      five coverage runs of `x11::`; and the launcher run by itself waits
      30 seconds by the clock for the program it starts (`fb49475`), where
      some five seconds of turns had been too few on a busy runner. Neither
      has failed in the nightly stress since, some 780 runs of the whole
      suite, nor in 400 runs of the launcher's, four at a time.
- [x] A program the compositor ends by its number, as a keeper retired
      after a new one takes over or a component that is to be started
      anew, could have ended already, and its number been given to
      another process, which would then have been sent `TERM` instead. It
      is now asked only while it is a child not yet waited for, whose
      number cannot be another's, or, kept from a compositor before,
      through the pidfd held for it.
- [ ] Protocols that let another's dock or bar manage windows and
      workspaces (`ext-workspace`, foreign-toplevel management). Waits on:
      people wanting to bring their own, as ours use the IPC socket.
- [ ] Programs written for X11 under WSL, where `/tmp/.X11-unix` is
      WSLg's and read-only: tried by hand on 22 November 2026, that was the
      one error, and glass was blurred, not solid, on llvmpipe and on
      Direct3D 12's Mesa alike. Meanwhile the guide says to start omorfos
      under `bwrap` with a directory of its own there (`docs/guide/wsl.md`).
      Waits on: Smithay listening on the abstract socket alone where the
      directory cannot be written to (`docs/UPSTREAM.md`). Tried by hand
      at the same time, a window under WSLg ended at its first frame where
      nothing changed, as Mesa will not swap a window it is not drawing
      into, and a blur had drawn elsewhere since; that is fixed.

A way in:

- [ ] A login screen of our own, in place of GDM, SDDM or LightDM: who
      is logging in, their secret, and which session. Long term. It is
      to be a greeter for `greetd`, which is the small service that
      checks the secret and starts the session, so that nothing of ours
      runs as root (ADR 35). It runs on the compositor itself, wears the
      design language, and shares what asks for a secret with the lock
      screen, and the on-screen keyboard (stage 7) with it.
- [ ] The lock screen and the login screen made from one template and
      one theme, so that what is unlocked looks as what was logged into:
      the same parts, the same layout of time, face, name and field, and
      the same `theme.css` rules for both. Each with a wallpaper of its
      own, set apart from the desktop's: the pictures, directories,
      slides, fit and fade of `wallpaper`, under a setting of their own,
      and the desktop's when that is not set. The lock screen's can come
      first, as it reads the user's configuration; the login screen's is
      the system's, in `/etc`, since the greeter runs as a user of its
      own and may not read anyone's home.

How the project is run, as learnt in part from Vela's:

- [x] Tests that need a graphics card reported as ignored, with that as
      the reason, where none is asked for, as in CI, and not as passed.
- [x] The workflows checked by `zizmor` in CI, for what lets a pull
      request or a release run what it should not.
- [x] `SECURITY.md`, a private way to report a vulnerability, and a
      form for reporting a bug that asks for what one needs.
- [x] The links between the documents checked in CI, without the
      network: each relative link in a Markdown file leads to a file or
      directory that is there, and to a heading in it where it names one,
      as GitHub makes them (`crates/omorfos-config/tests/links.rs`, among
      the tests every gate runs). Links elsewhere are not followed.
- [x] What a version promises written down (ADR 73, and in
      `RELEASING.md`): what someone else writes down or starts by name,
      the configuration's files and keys, the theme's parts, states and
      variables, the IPC protocol, `omorfosctl`'s commands, JSON and exit
      codes, the programs' and the IPC socket's names, and where the
      session's log is; not what only a person reads, nor the defaults,
      whose changes are said in the changelog all the same.
- [x] CI that finds a breaking change not marked as one, in the
      configuration: `schemas/promised.txt` lists each key by its path and
      each value by name, made from the schemas, and a commit that takes
      a line away from it without `!` or a `BREAKING CHANGE:` footer
      fails `Promises kept` (ADR 73).
- [ ] Known gaps in finding breaking changes: a key whose type narrows,
      as a number made a word, keeps its line; the theme's parts, states
      and variables, the IPC protocol and `omorfosctl`'s JSON are not
      listed yet.

What a user reads, asked for after trying the overview, so that what
there is can be found without reading the source (looked at on
2026-11-12: only `omorfos`, `omorfos-hold`, `omorfos-relay` and
`omorfosctl` answer `--help` with help, and two more with a line of
usage; there are no manual pages; the guide is `CONFIGURATION.md` alone):

- [x] Every program answers `--help` and `-h` with what it is, how it is
      started and what it takes, and `--version` and `-V` with its
      version, and does nothing else, wherever among its arguments it is
      asked: where `omorfos-panel`, `omorfos-launcher`, `omorfos-wallpaper`,
      `omorfos-notifications`, `omorfos-clipboard`, `omorfos-ask`,
      `omorfos-pinentry`, `omorfos-recording`, `omorfos-lock`,
      `omorfos-session`, `omorfos-screenshot` and the portal started as if
      run, `omorfos-lock` trying to lock the session. One way for all,
      `omorfos_ipc::help`, each program saying what it is, how it is
      started and who starts it, laid out as `omorfosctl`'s is: the
      commands and options of `omorfos-session`, `omorfos-screenshot` and
      `omorfos-lock` each with what it does, and examples, kept within 80
      columns; and a test of each that it answers, run where starting
      would fail.
- [x] `omorfosctl action` alone lists every action, in groups, with what
      it takes and what it does, from one list in `omorfos_ipc::actions`
      that a test holds to every action the configuration promises; and
      an action given one word, as `tile left`, is built as keys.yaml
      takes it, where it was refused.
- [x] A manual page for each program, `omorfos(1)`, `omorfosctl(1)` and
      the rest, and for the files a user writes in section 5,
      `omorfos-config(5)` for `config.yaml` and `omorfos-keys(5)`,
      `omorfos-rules(5)`, `omorfos-menu(5)` and `omorfos-theme(5)`, made
      from what `--help` and the schemas say, so that the two cannot
      disagree, and kept in `man/` by tests that make them again (ADR 77).
      Installed by `make install`, the `.deb`, the AUR package and the
      release's tarball, which a test checks, with a page for every program
      and every page a page points to. Doing so found the `.deb` without
      `omorfos-pinentry` and the tarball without `omorfos-hold` and
      `omorfos-relay`, now in both, which the same test checks.
- [x] The built-in bindings in `omorfos-keys(5)`, and the built-in menu in
      `omorfos-menu(5)`, before the keys, written as the files would write
      them, each binding with what its action does; a test reads each
      back as the same binding or menu. The schemas now give the defaults
      of the keys that take a number within bounds, which they had left out.
- [x] The guide lists the built-in bindings in a table of their own, made
      from them by the test that makes the manual pages, in place of the
      keys its table of actions named by hand.
- [x] Super is the desktop's (ADR 78): a chord with it that nothing binds
      reaches the program as Ctrl, as Cmd does on macOS (Ctrl+Shift in a
      terminal, by `keyboard.terminals`), unless `keyboard.super_as_ctrl`
      is off. The switcher on Super+Tab as well as Alt+Tab, the tabs of a
      group on Super+\`, fullscreen and new workspaces on Super+Ctrl, so
      that find and a new window are the program's, and the overview on
      the Mission Control key, F3 on a MacBook.
- [x] `keyboard-shortcuts-inhibit`: a virtual machine, a remote desktop
      or a game that asks is given every key, Super among them, while its
      window has the keyboard, with a notice; `Super+Escape` takes them
      back until it asks again (ADR 78).
- [x] The guide split into `docs/guide/`: a first page for the first
      login (what is there, the keys that matter, which a test holds to the
      built-in bindings, where the files are, how to bind what has no
      key), a page for each file and each part of the desktop, and what to
      do when something is wrong; installed whole by every packaging,
      which a test checks.
- [x] The guide's pages put where a user looks for them: `config.yaml`'s
      page holds its settings, and links to a page of its own for the
      screens and to the windows' page for workspaces, fullscreen and
      tiles, which it told of at length; the theme's page has the
      built-in theme folded away, so that it begins with what to change.
      The prose of each page is still as it was written.

Reviews, and what is still to decide (asked for on 14 November 2026):

- [ ] Whether the `unsafe` there still has to be: `omorfos-shm` and
      `omorfos-pipewire`, each against the safe ways its dependencies
      offer by then, with the checks of `docs/UPSTREAM.md` run; and each
      that can go, gone. Looked at on 15 November 2026: `omorfos-shm` has
      none since ADR 61, but reaches a program's memory through `/proc`
      for want of a safe write, asked of Smithay as Smithay#2288; the
      sixteen blocks of `omorfos-pipewire` all wait on the PipeWire
      bindings, asked of them as pipewire-rs#124. Open until either
      answers.
- [ ] The standards omorfos does not follow, and those it should take
      up: freedesktop.org's and Wayland's, the portals', the
      accessibility ones, the desktop entries' and the icon themes', the
      base directories' and the trash's, and the like, each listed with
      whether omorfos follows it, should, or has a reason not to.
- [ ] A review of the codebase, of how fast it is, and of how secure,
      by a stronger model than writes it, as Mythos 5.2 or Fable 5.2; what
      it finds fixed or put on this roadmap.
- [x] What other applications a desktop needs with it, beyond stage 7:
      decided on 15 November 2026, and added to stage 7: a calculator,
      one player for sound and video rather than two, a camera, a sound
      recorder, and a text editor last; a magnifier and a narrator are
      accessibility's, above.
- [ ] Stage 7's applications installable on any desktop, as GNOME, KDE
      or Xfce, and not only under omorfos: Files, the task manager, the
      image viewer, and those marked standalone in stage 7, packaged each
      by itself, needing nothing of omorfos's session.
- [ ] The configuration made clean: the defaults made really good and
      polished, with the review of the defaults (stage 4); then how the
      files are laid out and written. One `config.yaml`; a file for each
      part (`omorfos/bar.yaml`, `omorfos/files.yaml`,
      `omorfos/wallpaper.yaml`); or files read in the order of their
      names, each laid over the ones before it, as `00-config.yaml`,
      `01-config.yaml`, `99-config.local.yaml`; and whether Settings and
      the welcome card write the user's files at all, or keep what is
      chosen elsewhere until it is exported. The aim: not everyone cares
      for the files, and those who keep them, in git, want them changed
      only by themselves, holding nothing more than they set, so that a
      commit stays small. What is decided may change where ADR 81 has
      programs write. What is chosen is then exported, to standard output
      only, so that writing it to a file is the user's own doing, as
      `omorfosctl config export > ~/.config/omorfos/config.yaml` (the
      command's name and place to be settled): by default only what
      differs from the defaults, every default left out, so that the file
      stays small; with `--full`, everything, with comments, examples and
      the values each may take, for whoever wants the whole of it.

Reaching people:

- [x] The README put right: what omorfos is now, how to build, install
      and try it, and where the guide, the roadmap and the decisions are.
- [ ] Pictures of it in the README, once it wears the design language.
- [ ] A first release, which is also the first real test of the release
      pipeline (see [RELEASING.md](RELEASING.md)). Tried here first: the
      `.deb` and the tarball made from a release build as the workflow
      makes them, which found the tarball failing on the PAM service
      `omorfos-lock` beside the program of that name, now in `pam.d/`;
      and release-please set to start at 0.1.0, where it would have made
      a first release 1.0.0. What is left is the maintainer's: merging
      release-please's pull request, and watching the workflow build it.
      Not before the two high findings of the review of 26 November 2026
      are put right (above, under "Accessibility"): no release of any
      kind is made while either stands.
- [ ] After the first release: the gates run in CI, and `main` moved on
      by a merge queue only once they pass, so that a push no longer waits
      for the coverage gate on the maintainer's machine; a failure is then
      found some minutes after a push rather than before it (ADR 96,
      "Alternatives").
- [x] The shell in the user's language, Polish first: the mechanism
      (ADR 79), Fluent's messages in `i18n/<language>/<program>.ftl` of the crate, built
      in, the language the environment asks for with English beneath it a
      message at a time, and CI holding every language to the English (no
      message unknown to it, no value it is not given, every plural form
      the language has). The panel is the first program to speak through
      it, in English and Polish, the Polish by a machine and marked so.
- [x] The lock screen's words through `omorfos-i18n`, in English and
      Polish: its power buttons and what they ask, who keeps the machine
      from sleeping, and a password that was not right. What the fingerprint
      reader and a security key say is said in our words too: PAM's modules
      speak English to a program that has not set its locale, which needs
      `unsafe`, and pam_u2f has no translations at all.
- [x] The boxes that ask for the administrator's password and GnuPG's
      passphrases, through `omorfos-i18n`, in English and Polish, in the
      language the session is in as polkit is told it; what polkit asks
      for is polkit's, in the language it speaks.
- [x] `omorfos-session`'s notices through `omorfos-i18n`, in English and
      Polish: the volume, the microphone and the brightness, the radios and
      aeroplane mode, and a machine that woke unlocked.
- [x] The compositor's window menu, the pictures of where a window may
      go, and the question whether to keep the screens as the display key
      has them, through `omorfos-i18n`, in English and Polish.
- [x] The compositor's notices through `omorfos-i18n`, in English and
      Polish: of its own new versions, its locker and X server stopping,
      glass drawn solid, a program refused a role or given every key, and
      what the mark of a screen being recorded says. What it says of a
      mistake in the configuration, which names its keys and values, is
      English, as the configuration's own diagnostics are.
- [x] "About omorfos" and the list of problems through `omorfos-i18n`, in
      English and Polish: what the computer has, the headings of what
      omorfos is made with, its version and licence, and the button that
      copies, whose text for a bug report stays English.
- [x] The built-in root menu's labels in the user's language, which are
      the configuration's defaults, through `omorfos-i18n`, in English and
      Polish; and so an entry of `menu.yaml` named and doing just as one of
      them. The notifications program says nothing of its own but what
      programs send it.
- [ ] The manual pages and `--help` in the user's language, which are
      English for now.
- [x] The launcher showing applications by their names in the user's
      language, as their desktop entries give them (`Name[pl]`), and what
      they say of themselves (`Comment[pl]`), matched by the locale as the
      Desktop Entry Specification matches it. A name is found by what is
      shown; the English one, and the names the portal gives the programs
      that ask it for something, wait for the items they belong to.
- [ ] Someone who speaks Polish reads its files, and becomes their owner
      in `CODEOWNERS`.
- [x] A guide for someone who has just logged in for the first time, and
      a first run that is not an empty screen: `omorfos-welcome`, a card
      shown at the first login and from the menu's **Welcome**, with what
      the keys that matter do as `keys.yaml` has them, and the wallpapers
      that come with omorfos and light or dark to choose from, each saved
      to `config.yaml` as it is chosen (ADR 81), in English and Polish.
      It is drawn with the launcher's parts of the theme; parts of its own
      wait for a reason to look different. In steps, as the maintainer
      asked on 14 November 2026: a welcome with the mark, the keys,
      light, dark or with the day as small pictures of the desktop, a
      wallpaper, and the applications that open what, read from
      `mimeapps.list` and not yet chosen there; Skip, Back and Next on
      each, its buttons as a question's. The close button it had in its
      corner was taken out on 18 November 2026, as the maintainer
      asked: it looked out of place, and Skip does what it did.
- [ ] Packages beyond `.deb` and the AUR.

How functional the code is, and how solid that makes it, reviewed on
22 November 2026 as the maintainer asked
([reviews/2026-11-22-how-functional-it-is.md](reviews/2026-11-22-how-functional-it-is.md)):
a functional core inside an imperative shell, solid where the data is
modelled and fragile where a state is spread over fields that must
agree. Eight small faults were put right the same day, each in a pull
request of its own; what is larger, in the order worth taking:

- [ ] Medium: one flag, `fullscreen`, says both fullscreen and tiled, so a
      tiled window cannot go fullscreen as ADR 39 promises; an enum on
      `Group` and `Frame` in its place ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 1).
- [ ] Medium: each component's Wayland globals as one `Globals`, gathered
      once and naming what is missing, in place of 54 `Option`s ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 2).
- [ ] Low: one `Now` of the instant and the wall clock given to `refresh`,
      `deadline` and what starts a timer, so that nothing decides by
      reading the clock itself ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 3).
- [ ] Low: the X server's life as one enum in place of six `Option`s, and
      `manage_x11` a function from one state to the next ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 4).
- [ ] Low: the relay given the time in `step`, so its grace and its pause
      are tested without waiting ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 5).
- [ ] Low: the relay's maps of what is told of an object emptied by one
      `forget` ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 6).
- [ ] Low: the clipboard keeper deciding on its main thread alone ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 7).
- [ ] Low: states spread over pairs of fields made one each, in the relay,
      the portal, the screenshot, the panel and the holder ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 8).
- [ ] Low: a screen counted locked once its frame is shown, not drawn
      ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 9).
- [ ] Low: `clippy::indexing_slicing` turned on, crate by crate, from
      `omorfos-core` out ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 10).
- [ ] Low: `Config.scheme` and `theme` private, worked out in one place
      ([the review of how functional it is](reviews/2026-11-22-how-functional-it-is.md#on-the-roadmap), item 11).

What the coverage gate leaves out, reviewed on 23 November 2026 as the
maintainer asked
([reviews/2026-11-23-what-the-coverage-gate-leaves-out.md](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md)):
the seven files ADR 2 excludes are 2.7% of the code that is not tests,
and hold about seventy decisions ADR 2 says they do not, most in the
hardware backend; in the order worth taking:

- [ ] Low: nothing stops a decision being put into an excluded file
      again
      ([finding 11](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#11-nothing-stops-a-decision-being-put-into-an-excluded-file-again)):
      a test that counts the branches of each file ADR 2 excludes,
      parsed with `syn`, against a blessed budget, and fails where the
      coverage alias, ADR 2's table and the budget differ. First, so
      that the rest only bring the budget down.
- [ ] Medium: back from another virtual terminal, a screen that is to be
      off is not turned off again
      ([finding 2](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#2-back-from-another-virtual-terminal-a-screen-that-is-to-be-off-is-not-turned-off-again)):
      small: on activation, `dark` and `lit` are taken as not known, so
      the next turn applies them.
- [ ] Medium: a screen that could not be drawn on is not tried again
      until something else wakes the loop
      ([finding 1](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#1-a-screen-that-could-not-be-drawn-on-is-not-tried-again-until-something-else-wakes-the-loop)):
      what each screen is in as one plain type in `backend/mod.rs`,
      saying what to do; a failed draw sets the timer the tick does.
- [ ] Medium: what the session tells D-Bus and systemd as it starts,
      starts again and ends is not tested, nor that its values stay
      arguments to the shell
      ([finding 4](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#4-what-the-session-tells-d-bus-and-systemd-as-it-starts-starts-again-and-ends-is-not-tested-nor-how-the-values-reach-the-shell)):
      plain functions returning `Command`.
- [ ] Medium: how a wheel, a pointer that reports positions and a
      touchpad's gestures become input is decided only where no test
      reaches, though ADR 2 says it is covered
      ([finding 3](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#3-how-a-wheel-a-pointer-that-reports-positions-and-a-touchpads-gestures-become-input-is-decided-only-where-no-test-reaches)):
      `on_input` into a covered module, or first two plain functions;
      ADR 2's row put right.
- [ ] Low: a blur by the card's shader that fails leaves what it had
      drawn so far behind glass
      ([finding 9](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#9-a-blur-by-the-cards-shader-that-fails-leaves-what-it-had-drawn-so-far-behind-glass)):
      glass, which is covered, does not draw it.
- [ ] Low: the nested window's empty frame, added on 22 November, is
      decided where no test reaches
      ([finding 7](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#7-the-nested-windows-empty-frame-is-decided-where-no-test-reaches)):
      into a covered function generic over the renderer; ADR 5 to say
      whether a nested drawing error ends the session.
- [ ] Low: what is written to the log, and how often, is decided twice
      where no test reaches
      ([finding 6](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#6-what-is-written-to-the-log-and-how-often-is-decided-twice-where-no-test-reaches)):
      one `Compositor` method both backends write from.
- [ ] Low: which pointers are touchpads, and when settings are given
      again, is not tested
      ([finding 10](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#10-which-pointers-are-touchpads-and-when-settings-are-given-again-is-not-tested)).
- [ ] Low: a screen driven in a new mode, and the screens found, are
      given their size, scale, pace, order and names in two copies no
      test reaches
      ([finding 5](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#5-a-screen-driven-in-a-new-mode-and-the-screens-found-are-given-their-size-scale-pace-order-and-names-in-code-no-test-reaches)).
- [ ] Low: a layout of the card agreed but not to be had gives no
      memory, where its comment says something else is made
      ([finding 8](reviews/2026-11-23-what-the-coverage-gate-leaves-out.md#8-a-layout-of-the-card-agreed-for-a-shared-screen-but-not-to-be-had-gives-no-memory-where-its-comment-says-something-else-is-made)):
      to be decided which is meant; with the roadmap's item for a card
      in CI.

A review of the project from outside, of 23 November 2026, read from its
documents and not its code, asked for four things that hold when the code
is looked at; the rest it asked for was done already, or is for the
stages it belongs to:

- [x] The relay's reading of what programs send, and what it keeps of
      them, fuzzed: it reads every byte every Wayland program sends, and
      is the parser most open to them. `messages` fuzzed its reading and
      keeping already, message by message, but never what comes from a
      socket: a message in pieces, the descriptors beside it, and the
      bounds on what is kept to be written. `link` drives the relay's
      link for a program as the relay does, over sockets, with
      descriptors.
- [x] Arbitrary sequences of what can be done to windows, workspaces and
      screens, run against `omorfos-core` and the desktop checked to be
      whole after each: as a fuzz target, which needs no new dependency.
      `desktop` adds and takes away windows and screens, moves, tiles,
      arranges, attaches and focuses windows, and switches and slides
      workspaces, checking after each that every window is listed once,
      in a frame that holds it, on a workspace and a screen there are,
      that each screen shows a workspace it has, and that at most one
      window has the keyboard. Ten minutes found nothing.
- [x] Who may use the IPC socket, written down in `SECURITY.md`: any
      program of the user's outside a sandbox may have programs started,
      windows closed and the clipboard written, which is no more than it
      can do as the user, and a sandbox reaches the socket only if given
      `xdg-run`. Requests held to a role, should components of others
      come, to be weighed then. Written down under "Who is trusted",
      with what was found in writing it: a peer may `spawn` a program
      with any role, which is no more than writing `keys.yaml` gives it,
      and a sandbox given write access to `~/.config/omorfos` is as good
      as outside it.
- [x] How long frames take kept over time by the compositor, its middle
      and its slowest, for `omorfosctl frames` to say, in place of the
      last frame alone, which measuring glass on 23 November needed a
      sampler written for. Each screen keeps its last 256 frames' times,
      written over in a ring held once, and `frames` says the middle and
      the slowest of them beside the last; `omorfosctl frames` prints
      them.

The review of 23 November 2026, of the whole tree for safety, speed and
use, by eight reviewers at once, is written up in
[reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md),
in the order to take it; each that is put right is a pull request of
its own. It found one high, five medium and four low findings:

- [x] Medium: a lock screen started after a file has gone wrong uses the
      defaults, where the rest of the session uses what it kept
      ([finding 1](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#1-a-lock-screen-started-after-a-file-has-gone-wrong-uses-the-defaults-where-the-rest-of-the-session-uses-what-it-kept)):
      it starts from the kept copy, for what it offers and how it looks
      (#95).
- [x] Medium: a critical notification can be the one to go when there
      are too many
      ([finding 2](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#2-a-critical-notification-can-be-the-one-to-go-when-there-are-too-many)):
      the oldest that is not critical goes, while there is one (#100).
- [x] Medium: each refresh looks every window up in two lists, so a
      quiet turn costs as the square of the windows
      ([finding 3](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#3-each-refresh-looks-every-window-up-in-two-lists-so-a-quiet-turn-costs-as-the-square-of-the-windows)):
      the benchmark first. #102 looked two of them up in maps made anew
      each turn, and was taken back: a window over a framed one no longer
      eased where it was sent, and a turn with nothing changing took as
      long with it as without, measured in an optimised build: 70 µs
      with 32 framed windows and 0.9 ms with 256. The measure is kept, in
      `quiet.rs`, run by hand. Timed part by part, the two lists were not
      what grew: Smithay's `map_element`, which searches and sorts what
      the space holds for each window mapped, was a clean square (2.5 µs
      at 32 windows, 138 µs at 256); and whether a window is private,
      asked by its id twice for each, looked for its frame through every
      workspace. Windows are mapped again only when the space does not
      hold the scene's, in its order and where each was mapped; and how a
      frame is hidden is worked out once, from the frame, as the frames
      are known (`Desktop::hidden`). A turn with nothing changing, framed
      windows: 66 µs at 32, 0.72 ms at 256; windows drawing their own
      frames, 57 to 54 µs and 0.70 to 0.52 ms. Nothing tested that a
      window is told the screen it is on: with the space never mapped
      again, every test passed. One does now.
- [ ] What is left of that: three walks still go as the square of the
      windows, about 0.1 ms at 256 and nothing that shows at 32: the
      frame looked for by each window, the titlebar painted before looked
      for by each frame's tabs (`painted`), and, of windows drawing their
      own frames, whether each that was shown still is (`leave`). The two
      last are tied by `painted` taking what it finds out of the scene
      before, which is how #102 went wrong; changed only with a test of a
      window over a framed one, as #103's.
- [ ] A turn with nothing changing doing about 2.7 µs of work for each
      window, as each is eased where it is, its picture's stretch worked
      out, its titlebar asked whether it looks as it did, its program told
      how it is placed, and Smithay told which screens it is on: the third
      priority says no work when nothing changes, measured first.
- [x] Medium: what is captured is worked out, and written, rectangle by
      rectangle
      ([finding 4](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#4-what-is-captured-is-worked-out-and-written-rectangle-by-rectangle)):
      measured first, in an optimised build, with 1, 4 and 16 programs
      taking pictures of a window that says 1, 16, 256 and 4096 pixels
      changed. The cost followed the rectangles, about 0.8 µs each for
      each picture, a write of its own: 13 ms a frame with four programs
      and 4096; and sixteen programs were told 65,536 rectangles in one
      turn, more than their connection held, and were let go. Finding
      what is captured by session and screen was not borne out: sixteen
      programs and one rectangle cost 0.1 ms more than one, and is left
      as it is. A picture is now written in, and its program told, no
      more than sixteen rectangles: more are joined row by row, and then
      where they are least far apart, never beyond the rectangle round
      them all; four programs and 4096 take 2.4 ms, and sixteen 7.2 ms.
- [ ] What is left of that: each program taking pictures has Smithay's
      tracker of what changed go through every rectangle a window says,
      most of the 7.2 ms; one tracker for those taking the same drawing,
      measured first.
- [x] Low: the tests' Wayland client took a connection it could not read
      for one with nothing to read
      ([finding 5](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#5-the-tests-wayland-client-took-a-connection-it-could-not-read-for-one-with-nothing-to-read)) (#98).
- [x] Low: the shell's pictures handed to the compositor without
      checking that their size and bytes agree
      ([finding 6](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#6-the-shells-pictures-are-handed-to-the-compositor-without-checking-that-their-size-and-bytes-agree)) (#97).
- [x] Low: the relayed clipboard test's last assertion holds whatever the
      keeper does
      ([finding 7](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#7-the-relayed-clipboard-tests-last-assertion-holds-whatever-the-keeper-does)) (#99).
- [x] Low: three documents said what is not so
      ([finding 8](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#8-three-documents-said-what-is-not-so)) (#96).
- [x] Why the portal sometimes waits more than five seconds to end
      once the bus has gone, failing
      `it_ends_when_the_bus_does_and_needs_no_configuration_it_can_watch`
      in full runs
      ([seen in passing](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#seen-in-passing)).
      It took its name on the bus before most of its starting: joining
      the compositor, which starts the text and the keyboard's layouts,
      and loading the configuration and the theme twice. It notices the
      bus gone only in its loop, after all that, which took over four
      seconds in 106 of 200 runs sixteen at once. Taking its name last,
      once ready, none took over four, ten over two; alone it ends in
      a tenth of a second rather than a quarter.
- [x] High: a same-user process can make the sealed prompt ask for its
      passphrase
      ([finding 9](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#9-a-same-user-process-can-make-the-sealed-prompt-ask-for-its-passphrase)):
      decided on 24 November 2026 to keep ADR 58 as it is, which accepts
      it: the check on the pinentry's parent raises the bar and is not
      the boundary, as a program running as the user, outside a sandbox,
      is trusted ([SECURITY.md](SECURITY.md), "Who is trusted"). Telling
      the agent the user configured from one of its own would need a
      binding to it that GnuPG does not give, and would close one door
      of many such a program has.
- [x] Medium: the notification service advertises actions it does not
      provide
      ([finding 10](reviews/2026-11-23-the-whole-tree-for-safety-speed-and-use.md#10-the-notification-service-advertises-actions-it-does-not-provide)):
      preserve named actions, show them accessibly and emit the selected
      identifier; do not remove the capability as a stopgap. A program's
      named actions are kept, up to three shown as buttons under the
      words, drawn as `question-button`; the one chosen is told by its
      name, and the notification goes unless it is `resident`. The new
      action `focus_notifications` gives the newest the keyboard, which
      goes through it and its buttons; no key is bound to it until the
      defaults are reviewed.

The pull requests merged on 23 November 2026, #67 to #102, reviewed on
24 November by eight reviewers at once, with every gate run on `main`
as they left it, are written up in
[reviews/2026-11-24-the-pull-requests-merged-on-23-november.md](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md),
in the order to take it. `main` passes every gate with all of it in:

- [x] High: one region set on many surfaces is counted once by the
      relay and copied into each by the compositor, so one program can
      still fill the compositor's memory, #68's bound notwithstanding
      ([finding 1](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#1-one-region-set-on-many-surfaces-is-counted-once-by-the-relay-and-copied-into-each-surface-by-the-compositor-so-one-program-can-still-fill-the-compositors-memory)):
      each naming counted, and a program that takes a number withheld
      from it let go, by which the count could be got past
      ([the fixes, read again](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#the-fixes-read-again))
      (#104).
- [x] Medium: a window over one in a frame of ours no longer eases where
      it is sent, and every turn makes three maps anew
      ([finding 2](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#2-a-window-over-one-in-a-frame-of-ours-no-longer-eases-where-it-is-sent-and-every-turn-makes-three-maps-anew-that-it-then-throws-away)):
      #102 taken back, and the gate's ceiling lowered to what a turn
      allocates (#103).
- [x] Medium: a locked frame drawn and then refused by the card still
      counts its screen as showing nothing of the session
      ([finding 3](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#3-a-locked-frame-that-was-drawn-and-then-refused-by-the-card-still-counts-its-screen-as-showing-nothing-of-the-session)):
      not shown, on any failure to draw (#105).
- [x] Medium: the launcher, the screenshot tool and the welcome card
      still start from the files on disk, and nothing holds the lock
      screen, as it is started, to the copy
      ([finding 4](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#4-the-launcher-the-screenshot-tool-and-the-welcome-card-still-start-from-the-files-on-disk-and-nothing-holds-the-lock-screen-as-it-is-started-to-the-copy)):
      one way in for every program but the compositor, and a test that
      reads each program's source for one that opens the configuration
      without the copy (#106).
- [x] Low: an ordinary notification that comes while eight critical ones
      show is itself the one to go
      ([finding 5](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#5-an-ordinary-notification-that-comes-while-eight-critical-ones-show-is-itself-the-one-to-go-and-is-never-seen)):
      what has just come is never the one to go (#107).
- [x] Low: a configuration directory's changes still missed, or
      mistaken, four ways
      ([finding 6](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#6-a-configuration-directorys-changes-are-still-missed-or-mistaken-four-ways)):
      three of the four, with what sleeps until its watcher can be
      read woken to look again for a link's directory; the directory
      above said in ADR 3 (#108).
- [x] Low: what every peer asks in a turn read into memory before any of
      it is answered
      ([finding 7](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#7-what-every-peer-asks-in-a-turn-is-now-read-into-memory-before-any-of-it-is-answered)) (#109).
- [x] Low: six of the day's fixes held by no test
      ([finding 8](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#8-six-of-the-days-fixes-are-run-by-the-tests-and-held-by-none)):
      #69 in #108, #85 in #105, the other four in #110.
- [x] Low: a program let go for what it added in all told it added too
      much to one object
      ([finding 9](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#9-a-program-let-go-for-what-it-added-over-all-its-objects-is-told-it-added-too-much-to-one)) (#104).
- [x] Low: the check of `omorfos_shell::pixels` that function's alone
      ([finding 10](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#10-the-check-97-gave-omorfos_shellpixels-is-that-functions-alone)):
      taken by all five, and by the stream's own shared memory (#111).
- [x] Low: what the review of 23 November says that is not so
      ([finding 11](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#11-the-review-merged-in-101-says-six-things-that-are-not-so)):
      put right with this write-up.
- [x] Low: documents and comments the day's changes left behind
      ([finding 12](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#12-documents-and-comments-the-days-changes-left-behind)):
      the three reviews with this write-up, and the rest in #109, #113
      and #114.
- [ ] Low: a pull request's fix-ups squashed before it is rebased onto
      `main`, and why `committed` passes a subject of 74
      ([finding 13](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#13-six-commit-messages-do-not-hold-to-agentsmd-or-do-not-stand-alone-on-main)).
- [x] The gates' `commit` tier building `omorfos-pinentry` where it tests
      `omorfos` in a build folder with nothing in it
      ([seen in passing](reviews/2026-11-24-the-pull-requests-merged-on-23-november.md#seen-in-passing-for-later)) (#112).

## 9. For others to build on

Asked for on 14 November 2026: a way for other projects and repositories
to plug into omorfos, as a bar, an applet or the look of windows, through
an integration layer that shares what omorfos's own components are built
with, rather than each copying it. The open standards come first, as
everywhere: what a Wayland protocol, a portal or a freedesktop.org
specification already says is not said again in a way of omorfos's own.

- [ ] First, what is made more than once made once (asked on
      15 November 2026), so that what is offered is one way of each thing
      and not several, nor a copy frozen by a promise. The components were
      written one at a time, and each made its own of what they share:
      buttons, drawn by the lock screen, the boxes that ask for a password,
      the welcome card, the panel, the portal's question and "About", as
      the theme's `question-button` and `about-button`; fields typed in,
      `omorfos_ui::Field` in the launcher only, the lock screen and the
      boxes for a password their own, as `launcher-input` and
      `question-input`; what dims or blurs the rest of the screen behind
      something, as `launcher-backdrop`, `switcher-backdrop`,
      `overview-backdrop` and `sealed`; and boxes that ask or tell, the
      question, "About", the notice, the welcome card and the lock screen,
      each laid out in its own way. A review of the components finds each
      such thing; each becomes one widget in `omorfos-ui`, drawn as one
      part of the theme with its states, that every component takes up,
      the parts it replaces kept as names for it until a major version, as
      ADR 73 has it; and what is left apart says why. Begun on
      15 November 2026 with the buttons and the boxes that ask (ADR 92):
      one button and one box in `omorfos-ui`, taken up by the leave
      question, the portal's question, the welcome card, the lock screen
      and the password box's buttons. Next: the password box's layout, as
      a change of its own; the fields typed in; and the backdrops.
- [ ] The one button and the one field, further (asked on 17 November
      2026, as the buttons were tried by hand), for every button and
      field omorfos draws, in the compositor's parts and in the
      components alike:
      - a button may hold an icon, and is no larger or smaller for
        holding one;
      - the pointer changes over a button that can be pressed, changes
        again while it is pressed, and is the shape that says "not
        allowed" over one that is blocked, which a press does not change.
        No shape is named for a button pressed, so it is one of the
        theme's own, with another to fall back on where a theme has
        none;
      - the pointer is the shape of text over a field that can be typed
        into, and "not allowed" over one that cannot.

      What other programs, games and pages show over their own windows
      stays theirs to say.
- [ ] Whether, and what: an ADR on what is offered to others and how
      firmly it is promised. Nothing loaded into the compositor's own
      process: a plugin there would run with every key and every window
      within its reach, and Rust offers no stable way to load one without
      `unsafe`, against the first priority (and ADR 18, that what can be
      a program of its own is one). What others build runs as a program,
      over the protocols and omorfos's own socket (`docs/IPC.md`, already
      promised as the configuration is).
- [ ] The crates omorfos's components are built with offered to others,
      with a promise of what does not change without a major version: as
      `omorfos-ui` (drawing), `omorfos-config` (the theme and the
      configuration), `omorfos-shell` (a component's Wayland plumbing and
      its starting again), `omorfos-i18n` (words in the user's language)
      and the IPC client, so that another repository's bar or applet
      looks and behaves as omorfos's own, started and kept as they are
      (ADR 28, ADR 60).
- [ ] What is not yet a standard, raised where it could become one
      (ADR 99, [STANDARDS.md](STANDARDS.md)), each read up on first and
      joined where someone has proposed it already, by the maintainer
      or an agent asked to:
  - [ ] the side and order of a titlebar's buttons, as a setting of the
        portal's own in place of GNOME's `button-layout`;
  - [ ] the interface's font and the monospaced one, likewise;
  - [ ] the pointer's theme and size, if a setting is still wanted
        where programs name shapes;
  - [ ] the icon theme in use;
  - [ ] less transparency, as a key beside `reduced-motion`;
  - [ ] layer-shell, or what follows it, in wayland-protocols, which
        every small desktop's shell stands on and GNOME does not offer;
  - [ ] raising and closing another program's windows, with the dock;
  - [ ] a window's picture shown by a program that is not given it, as
        a dock's previews, a taskbar's, a switcher's or an overview's,
        and what chooses a window to share: once the dock is built;
  - [ ] controls in a titlebar: what became of the 2022 proposal to
        xdg-specs, read before stage 7;
  - [ ] typing without a keyboard and input methods, with the on-screen
        keyboard;
  - [ ] what a protocol for a session that outlives its compositor
        would have to carry, written up from what the relay remembers.
- [ ] Programs that stand alone (ADR 98), the first of them: the lock
      screen, the wallpaper and `omorfos-clipboard` made to run where
      omorfos is not, what little they ask of it made optional, each
      with settings and a theme it finds by itself, tried on one other
      compositor and packaged apart. The applications of stage 7, the
      greeter, the dock, Launchpad and the on-screen keyboard are built
      so from the start; `omorfos-pinentry` stands alone unsealed. Each
      with a version, a changelog, a tag and a release of its own, the
      licences of what is in it and no more, its licence and credits in
      its manual page, a README for whoever finds it without the rest,
      the release workflow building only what it releases, and CI only
      what a change reaches (ADR 102).
- [ ] Settings that stand alone and change whatever of omorfos's is on
      the machine, and nothing else (ADR 98): a page for each program of
      ours that is installed, made from its schema, and what is not the
      user's own to change, as the greeter's settings, locked until
      they unlock it. How, with nothing of ours left running as root,
      is for an ADR of its own and a review for security first.
- [ ] The look of windows offered as themes, not code: what stage 4's
      redesign settles is made of the theme's parts, so that another
      project's decorations are a stylesheet laid over the built-in one.
- [ ] A guide for whoever builds on it, with a component written outside
      this repository as its worked example (a widget on the desktop, as
      a clock, being the obvious one), run by CI against each
      release so that a promise broken shows.

## Done

In the order it was built. The decisions are in [adr/](adr/), and the
detail of each step is in the history of this file.

- **Bootstrap.** The workspace, lints, the coverage gate, CI.
- **Configuration.** YAML files with defaults, validated with file, line
  and column; the last good version of each kept; watched and reloaded as
  they are saved; JSON Schemas generated (ADR 3).
- **The desktop model.** Windows, screens, workspaces, focus, stacking,
  placement, tabs, key chains and rules, knowing nothing of Wayland
  (ADR 1).
- **The compositor.** On Smithay: xdg-shell, input, key bindings,
  damage-tracked drawing, and a harness that tests it through a real
  client with a software renderer.
- **Releases.** From commit messages, for amd64 and arm64, as a `.deb`
  and for the AUR. Not yet exercised by a real release.
- **Frames.** Frames with tabs drawn by the compositor, moving
  and resizing, the root menu.
- **IPC and protocols.** The socket and `omorfosctl`; layer-shell; the
  clipboard and the primary selection; popup grabs; activation; the list
  of windows for taskbars; problems shown on screen.
- **Real hardware.** DRM, libinput and libseat (ADR 9, 10); several
  screens, each with its own scale and its own workspaces (ADR 11, 15,
  17); the pointer (ADR 12); a session for login screens (ADR 13); idle,
  inhibiting it, and locking by another's locker (ADR 14); rounded
  corners (ADR 16).
- **Shell components** (ADR 18): a shared drawing crate; the panel with
  workspaces, windows and a clock; the launcher (ADR 22); notifications
  over D-Bus (ADR 23); the theme as parts and states in CSS (ADR 19, 20,
  21).
- **Movement.** Colours that change over time (ADR 24); what comes onto
  the screen fading in and what goes fading out (ADR 25, 26).
- **Programs written for X11** (ADR 27): framed and managed like any
  other, started again if the X server stops, copying and pasting with
  programs written for Wayland, and drawn sharply on a dense screen for
  whoever asks (ADR 30).
- **Looking after itself.** The panel and the notifications started again
  if they crash (ADR 28); what is wrong shown among the notifications,
  and what merely happened told apart from it (ADR 29).
