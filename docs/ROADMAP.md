# Roadmap

What omorfos is to become, in the order it is to be built. Stages are
worked top to bottom and one at a time; within a stage the order is a
suggestion. Tick items as they land and keep the "Next" pointer current.

**Next:** stage 3, what a day's use needs. Of stage 1 only pushing the
work and seeing it pass in CI is left, which the maintainer does.

Items that came from the wishlist kept outside this repository carry its
number, as (wish 3), so that nothing asked for is lost track of.

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

## How the stages were ordered

1. What takes a day and is felt every day comes first.
2. Decisions that many later things rest on are made before those things,
   each as an ADR, and cost little.
3. What stops omorfos being used all day comes before what makes it
   pleasant: sharing a screen in a call matters more than a dock.
4. The look comes before the shell that is to wear it.
5. Applications come last, because they are the most work, the least
   bound to omorfos, and need the toolkit decision of stage 2.

## 1. Quick wins

Small, independent, and each an improvement the day it lands.

- [x] The launcher on `Super+Space` (wish 2). The root menu, which had
      that key, is on `Super+Escape` until the bar has its menu.
- [x] Mouse and touchpad set apart (wish 4): the touchpad scrolls the
      natural way by default and the mouse does not, each with its own
      setting.
- [x] Pointer speed and acceleration, for the mouse and for the touchpad.
- [x] Titlebar buttons: close, maximize, minimize, at the left, coloured,
      each a part of the theme (wish 3). Double-click on a titlebar
      maximizes.
- [x] A menu on a right click of the titlebar (wish 3): the same three,
      and shade (roll the window up into its titlebar), keep above the
      others, show on every workspace, send to a workspace (ADR 31). The
      first three of those are also actions, to bind keys to: `shade`,
      `keep_above` and `stick`.
- [x] What a window is kept as, told to panels and taskbars over IPC and
      set by rules: shaded, above, on every workspace.
- [x] The root menu, and the titlebar's, driven from the keyboard:
      arrows, Enter, Escape.
- [ ] The work pushed and seen to pass in CI. At the time of writing 57
      commits are on this machine only, and CI has not run on any of
      them: the XWayland tests have never run anywhere else.

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
- [ ] Not at all while a program holds the pointer, once programs can
      (`pointer-constraints`, below).
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
- [ ] Icons beside the titles of windows offered in an empty part, and
      in place of a private window's blank (ADR 38), once there are icons
      from the icon theme.
- [ ] Windows made small by halves (ADR 38), once the renderer for
      real hardware can draw from what it drew: before the switcher.
- [x] A window that nobody asked for, opening while a fullscreen
      workspace is shown, left on the desktop unseen (activation
      tokens).
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
      screen, and `Print` starts it. The one `unsafe` block in omorfos
      is here (ADR 40).
- [x] A picture put on the clipboard instead of, or as well as, saved:
      `screenshots.destination`, `--to`, and `Shift+Print`. With it the
      `clipboard` role and `ext-data-control-v1`, since a program
      without a window cannot otherwise set the clipboard.
- [x] A clipboard that outlives the program that copied: a manager of
      our own with the `clipboard` role, which would also let
      `omorfos-screenshot` stop at once (ADR 42).
- [ ] What `omorfos-clipboard` keeps, offered as a history to choose
      from (ADR 34 has its look).
- [ ] Forgetting what was copied after a while, for passwords copied
      from where nothing says they are secret: Proton Pass in Firefox
      clears what it copied only while its popup stays open.
- [x] The `private` rule, which leaves a window blank in any picture
      taken of the screen, for as long as it matches (ADR 38).
- [x] Pictures of one window (`ext-foreign-toplevel-image-capture-source`),
      and the screenshot tool taking one of the window with focus, with
      `--window` and `Alt+Print`.
- [x] Pictures of a window as large as it is, and see-through where it
      is, saved so by `omorfos-screenshot --window`.
- [ ] Known gaps in pictures of a window: it is given without the frame
      omorfos draws, with no way to ask for it; and one taken in the
      memory of the graphics card is solid, see-through only in shared
      memory.
- [ ] A private window blurred in pictures in place of blank, as a
      choice of the rule (`private: blur`), once glass has brought the
      blur and a way to draw a window by itself. A blur hides less than
      a blank does, which the guide is to say.
- [x] A private window's last moment, as it fades when closed, left out
      of pictures as the window was.
- [ ] Known gaps in what is private: a notification about a private
      window, and its menus if it is written for X11, are in a picture;
      and a program cannot yet say of its own window that it is private.
- [ ] Sharing the screen in Firefox froze once, on real hardware, just
      after a private window opened, and not again with every picture
      logged: what failed, if anything, is not known.
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
- [ ] A layer surface with a subsurface that reaches past its screen is
      not drawn at all, found as the picker's line of what to do was
      made, which is now kept within the screen. To be found out and
      put right in the compositor.
- [x] A screen that changes size goes on being shared at its new size:
      the stream offers it, what reads it agrees on it again, and is given
      memory made to it, of the card or shared.
- [ ] A screen changing size while it is shared has only been tried by
      the tests, which resize a screen of their own, and a shared window
      changing size by hand in Firefox. It is to be tried by hand with a
      screen, now that its `mode` can be changed while it is shared.
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
- [ ] The portal's settings read from `appearance` once there is one
      (stage 4), with reduced motion and high contrast; today light or
      dark is worked out from the theme's colours.
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
      server started and as it was started again. It needs a request
      that does not wait.
- [x] An arrangement of the screens for each set that may be plugged in
      (`arrangements`), followed as screens come and go.

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
      not by halves, so text in them is rough; and the pointer moving
      over a window does not pick it.
- [x] Sleep, restart and shut down, by logind, and what the lid and the
      power button do. Sleep, by the lid or asked for, locks the screen
      first, with the locker in use until `omorfos-lock` takes its place,
      and waits for it to be locked, so that the machine never wakes
      unlocked. `omorfos-session` runs all session to do it, and
      `loginctl lock-session` locks too. The lid and the keys for sleep
      and for turning off are logind's, which already acts on them, so
      they are not bound (ADR 35).
- [ ] Sleeping when the user is away, after a time the configuration
      gives, as ADR 35 has it: today only the screens go off, and an idle
      daemon is needed to lock.
- [ ] The radios, wireless, Bluetooth and the rest, known to omorfos
      whatever turned them off or on: a key the kernel handles itself (on
      a ThinkPad, `thinkpad_acpi`), a switch on the side, or another
      program. Each change is shown as the volume is, "Bluetooth off",
      with an icon for it; and the panel shows which are off, aeroplane
      mode when all are, from the compositor's IPC state, which gains the
      radios and says when they change. The keys are not bound: the
      kernel has already acted on them, and a binding as well would undo
      it. The radios are read from `/dev/rfkill`, which tells of each
      change as it happens, by the part of `omorfos-session` that runs
      all session, which tells the compositor.
- [x] Writing in languages that need an input method
      (`text-input-v3`, `input-method-v2`, `virtual-keyboard-v1`), for a
      program given the `input` role, its popup under the text and no key
      shown it while locked (ADR 47); and switching keyboard layout by
      the `next_layout` action, which no key is bound to.
- [ ] The layout shown: which one is being typed with, on the panel, and
      a key for `next_layout` decided with the other default keys.
- [ ] A second input method refused, not the first: Smithay tells the one
      there already that it may no longer be one.
- [x] Programs listed in `~/.config/autostart` started with the session,
      and the session's environment told to D-Bus and systemd, without
      which portals and much else do not find it. The environment was
      told already; the entries are started once at login by
      `omorfos-session autostart`, the system's too, as the specification
      has it.
- [ ] A polkit agent, so that a program can ask for the administrator's
      password, and `omorfos-pinentry` for keys and passphrases
      (wish 9): the screen dimmed and blurred behind one prompt, what is
      typed hidden unless asked to be shown. Both take the keyboard
      exclusively, as the lock screen does. Early, because without an
      agent a program that asks for the administrator fails without
      saying why.
- [ ] Wallpaper (wish 6): a colour or a picture per screen, filled,
      fitted, stretched, centred or tiled; a directory or several shown
      in turn, in order or shuffled, at an interval. A program of its
      own, which the theme's `desktop` gives way to.
- [ ] Screens arranged by the user: mirroring. The display key
      (`XF86Display`) goes through the screens side by side, the same on
      both, and only one of them, each kept only if the user says so in
      time, as the next item has it, since it is the key pressed when a
      screen has gone dark.
- [ ] A change to the screens made from Settings (stage 7) kept only if
      the user says so in time, and put back by the compositor itself
      otherwise, so that a screen left dark comes back on its own.
- [ ] A lock screen of our own, `omorfos-lock`, in place of relying on
      another's: started by a key, by being away, by the lid and by
      logind. It is the most security-critical program here after the
      compositor and is written and reviewed as such. The on-screen
      keyboard (stage 7) is to be usable on it.
- [ ] Smaller protocols that programs complain of lacking:
      `xdg-foreign`, `presentation-time`,
      `keyboard-shortcuts-inhibit`, content type and tearing control.
- [ ] What games and remote desktops need of the pointer: locked,
      confined, and moved without a position (`pointer-constraints`,
      `relative-pointer`).
- [ ] Gestures on the touchpad: workspaces by a swipe of three fingers.

## 4. The look

Stage 2 says what it is to be; this builds it.

- [ ] Shadows under windows, menus and panels (`box-shadow`), as
      pictures made on the CPU (ADR 33).
- [ ] Glass, drawn in software and tested (`backdrop-filter: blur()`):
      what is under each sheet, its blurred picture, when that is stale,
      and the software blur that is the shader's specification. The
      wallpaper is blurred once and used wherever nothing else is under
      the glass. The hardware backend draws solid until the next item.
- [ ] The blur shader, its check at startup, and the file left out of
      coverage, tried on real hardware: behind the bar, the dock, menus
      and notifications, for the compositor's parts and the components'.
- [ ] How long a frame took to draw, and how many blurs it made, in the
      state published over IPC.
- [ ] Blur behind titlebars, once its cost has been measured with a
      dozen windows on a modest laptop.
- [ ] Blur for other programs that ask (`ext-background-effect`).
- [ ] Gradients and a picture as a background, a transition's delay, and
      `cubic-bezier()`.
- [ ] The built-in theme redrawn in the design language: light and dark,
      with the accent a setting, and the settings under `appearance`.
- [ ] `appearance.scheme: auto`: light from sunrise to sunset, worked
      out from a latitude and longitude the user gives, or fixed hours
      without one.
- [ ] The default font and icons, and a pointer that is the same size
      everywhere, including over programs written for X11.
- [ ] A pointer of our own, built in as shapes so that it is sharp at any
      size and there when no cursor theme is installed.
- [ ] "About" shows `CREDITS.md`, and packages carry the licence texts
      it names.
- [ ] Icons in menus: a glyph before each entry of the titlebar's menu
      and of the root menu (a program's own icon there), and a tick
      beside what is on in place of wording that changes. It waits for
      the icon set stage 2 chooses.
- [ ] Movement: windows that open and close with a slight change of size
      as well as of opacity, workspaces that slide, a window that goes to
      the dock when minimized, a notification that comes in and goes
      beside others that stay; the panels and the titlebar sliding in
      from the edge they are brought back at on a fullscreen workspace,
      and out again. All of it off with one setting.
- [ ] To be tried once there are effects: on a shared workspace, the
      windows without focus drawn a little fainter or blurred, so that
      the one with focus stands out. It has to be weighed against the
      design language, which keeps a window's content from being glass
      because it is there to be read.
- [ ] The switcher as a queue on a diagonal (wish 5).
- [ ] An overview of every window on a workspace, and of the workspaces.
- [ ] Night light: the screen warmer after dark.

## 5. The shell: a bar and a dock

The panel becomes two programs (wish 1). Each is a client of the
compositor as the panel is now, started and started again in the same
way (ADR 28). The panel that is there today goes when both are done.

What both need first:

- [ ] Menus and small windows that hang from a bar: a popover with
      content, placed by the compositor, closed by a click elsewhere.
- [ ] Tooltips.
- [ ] Icons from the icon theme in use, at any size and scale.

`omorfos-bar`, along the top, see-through with enough of a tint to be
read over any wallpaper. What is on it is fixed; how each thing behaves is
a setting.

- [ ] The omorfos menu, at the left, its entries from `config.yaml`. By
      default: Run, About, Settings, Task Manager, Sleep, Restart, Shut
      Down, Lock Screen, each with its key beside it. Entries whose
      program is not there yet are left out, not shown and dead.
- [ ] The clock, by default as "Wed Apr 1 9:41 AM", its format a setting.
      A click opens a month, walked a month at a time.
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

- [ ] Launchpad (wish 9): every installed program on a full screen, as a
      grid or a list, in pages, arranged by dragging, with folders that
      can be named.
- [ ] A clipboard with a history (wish 9): what has been copied, to pick
      from, each thing forgotten after a time that is a setting. It
      needs a privileged protocol (`ext-data-control`), honours what
      password managers mark as secret, and keeps nothing on disk unless
      asked.
- [ ] The launcher: looser matching, names in the user's language, and
      windows to go to as well as programs to start.
- [ ] Sums in the launcher: what is typed is worked out as it is typed,
      and Enter copies the answer. Worked out by code of ours; nothing
      typed is ever run.
- [ ] Files in the launcher: those in the home directory that are not
      hidden, found by name and opened with the program for their kind.
      Which directories are looked in is a setting, and nothing is kept
      on disk about them.
- [ ] Shortcuts in the launcher: a word, and what to do with the rest of
      what is typed. "google" with a question opens the default browser
      at a search for it; "ai" opens an assistant, in the browser or a
      program, with the question. Each is an entry in the configuration:
      an address with a place for the question, or a program and its
      arguments. The question is put into an address encoded and given
      to a program as one argument, never through a shell. A few come
      set up; the user adds their own.
- [ ] Hot corners and edges, if wanted.

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
- [ ] **Settings** (wish 7): the common choices of `config.yaml` and the
      theme, written to the files as they are made and taking effect at
      once, since the files are already watched. It writes what a person
      could have written, keeps what it does not understand, and offers
      presets. Few options, well chosen; everything else stays in the
      files. The keyboard's settings key (`XF86Tools`) opens it.
- [ ] **Files** (wish 9): places and starred places at the side; volumes
      mounted when plugged in, and unmounted; list, details and
      thumbnails; make, rename, move, delete to the trash; archives
      opened; a quick look at a picture or a document on Space. Remote
      and cloud storage as plugins behind one interface that is designed
      at the start and filled in later. This is the largest single thing
      on this page.
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
- [ ] About: what the system is, shown from the bar's menu.
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

## 8. Reach and hardening

Ongoing, and taken between the stages above as much as after them.

Security:

- [ ] The configuration, theme and IPC parsers fuzzed.
- [x] Sandboxed programs recognised and held to less
      (`security-context-v1`): done in stage 3.
- [ ] What a sandbox says its program is (the application's name) kept
      with the connection, for the portals to ask the user by.
- [ ] Smithay asked for a safe way to write into a buffer a program
      shares, so that the one `unsafe` block can go (ADR 40).
- [ ] A safe way offered to the PipeWire bindings to hold a stream's
      buffer and to give one memory of the caller's own, so that
      `omorfos-pipewire` can go (ADR 41). Wanted soon.
- [ ] A review of everything a client can make the compositor allocate or
      wait for.

Accessibility:

- [ ] Everything in the shell reachable from the keyboard.
- [ ] High contrast, larger text and reduced motion, as settings the
      portal also tells applications of.
- [ ] A magnifier.
- [ ] Sticky and slow keys.
- [ ] What a screen reader needs of the shell (AT-SPI), which depends on
      the toolkit decision.

Hardware and performance:

- [ ] A window that fills the screen shown without being copied (direct
      scanout), and the refresh rate following it (VRR).
- [ ] Touch screens and drawing tablets.
- [ ] Wide colour and HDR, when Smithay can.
- [ ] Measured: what an idle session costs, and how long a frame takes.
- [ ] The third priority as a gate: a test that a session with nothing
      changing draws nothing and allocates nothing, turn after turn.
- [ ] What is claimed of speed made by a script, and each number kept
      with the commit, the card, its driver and how many runs it is of.

Reports and recovery:

- [ ] `omorfosctl report`: the versions, the configuration, what the
      compositor and its programs wrote, and the state the IPC socket
      gives, gathered into one file for the user to look over before
      handing it on. Titles and paths are left out unless asked for, and
      nothing is sent anywhere. Every report from real hardware so far
      has needed a log found and sent by hand.
- [ ] A compositor that crashes started again with the session kept: a
      small program holding the Wayland socket, so that programs can
      connect again, and a session that was locked comes back locked.

Programs written for X11:

- [ ] Dialogs kept with the window they belong to, and docks, splash
      screens and the like treated as such.
- [ ] The X server started when first asked for. This waits on a change
      in Smithay, which is worth offering to it, as is the fix for what
      it leaves unsent (ADR 27).

Known gaps carried forward:

- [ ] During a resize from the left or top edge, the window's content can
      lag the frame by a frame or two.
- [ ] Dragging and dropping between applications works through Smithay's
      defaults and has no test.
- [ ] The nested backend has not been run by hand since the event loop
      moved into `Session`.
- [ ] One branch for programs written for X11 (something asked of a
      window the compositor does not manage yet) is reached by the tests
      only by timing, so the coverage gate has been seen to miss it once.
      A test is to bring it about for certain. Likewise the test of the
      launcher run by itself has failed a few times in some hundreds of
      runs: the launcher shows, Enter is pressed, and the program picked
      is never started. It could not be made to happen again in two
      hundred and fifty runs with the test saying more, so the test now
      says what the launcher and the compositor made of it when it does.
      It did once more, under the coverage run: the launcher had sent its
      request and ended well, and the compositor had no problem to tell.
      So the request was sent, and either it was not read or the program
      started from it never ran.
- [ ] Protocols that let another's dock or bar manage windows and
      workspaces (`ext-workspace`, foreign-toplevel management). Not
      planned while ours use the IPC socket; to be looked at again if
      people want to bring their own.

A way in:

- [ ] A login screen of our own, in place of GDM, SDDM or LightDM: who
      is logging in, their secret, and which session. Long term. It is
      to be a greeter for `greetd`, which is the small service that
      checks the secret and starts the session, so that nothing of ours
      runs as root (ADR 35). It runs on the compositor itself, wears the
      design language, and shares what asks for a secret with the lock
      screen, and the on-screen keyboard (stage 7) with it.

How the project is run, as learnt in part from Vela's:

- [x] Tests that need a graphics card reported as ignored, with that as
      the reason, where none is asked for, as in CI, and not as passed.
- [x] The workflows checked by `zizmor` in CI, for what lets a pull
      request or a release run what it should not.
- [x] `SECURITY.md`, a private way to report a vulnerability, and a
      form for reporting a bug that asks for what one needs.
- [ ] The links between the documents checked in CI, without the
      network.
- [ ] What a version promises written down: what counts as breaking,
      which, as nothing here is a library others build on, is what users
      and other programs depend on: the configuration's keys, the
      theme's parts and states, the IPC protocol, `omorfosctl`'s
      commands and what it prints, and the names of the programs.
- [ ] CI that finds a breaking change not marked as one: a schema of
      the configuration that loses or renames something in a commit
      without `!`.

Reaching people:

- [x] The README put right: what omorfos is now, how to build, install
      and try it, and where the guide, the roadmap and the decisions are.
- [ ] Pictures of it in the README, once it wears the design language.
- [ ] A first release, which is also the first real test of the release
      pipeline (see [RELEASING.md](RELEASING.md)).
- [ ] The shell in the user's language, Polish first. The mechanism and
      the English are part of this repository, so that CI can test every
      program in every language; each language's words are files of
      their own (`i18n/<language>/`), with an owner each in `CODEOWNERS`,
      so that translating needs no review of code. English is always
      loaded, and a message missing from a language, or written with the
      wrong placeholders, is shown in English, a message at a time. CI
      checks every language against the English: no message unknown to
      it, the same placeholders, and every plural form the language has.
      A translation made by a machine is marked as such until someone
      who speaks the language has read it. To be decided as an ADR: the
      format (Fluent, which has per-message fallback and Polish's plurals
      built in, or gettext, which translators' tools know best).
- [ ] A guide for someone who has just logged in for the first time, and
      a first run that is not an empty screen.
- [ ] Packages beyond `.deb` and the AUR.

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
