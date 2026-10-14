# Roadmap

What omorfos is to become, in the order it is to be built. Stages are
worked top to bottom and one at a time; within a stage the order is a
suggestion. Tick items as they land and keep the "Next" pointer current.

**Next:** stage 1, quick wins.

Items that came from the wishlist kept outside this repository carry its
number, as (wish 3), so that nothing asked for is lost track of.

## Where this is going

omorfos began as a Fluxbox for Wayland: small, fast, driven by text files.
It still is small and fast and driven by text files, but what is being
asked of it now is a whole desktop: a bar, a dock, a way to switch
windows, settings, and programs to start with. It should look modern, in
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

The description of the project as "inspired by Fluxbox" is to be
revisited when stage 5 is done and it is no longer the whole truth.

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
- [ ] What a window is kept as, told to panels and taskbars over IPC and
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
- [ ] The same by pushing against the edge, without the wait, on a real
      mouse or touchpad; and not at all while a program holds the
      pointer, once programs can.
- [x] Tiled workspaces (ADR 39): a workspace made for up to four
      windows that fill the screen between them. First the model, with
      two windows side by side made from the titlebar's menu and by a
      key, each told it is tiled, and a window that leaves giving its
      room up.
- [x] What is brought back at an edge staying while the pointer moves,
      so that a titlebar away from the edge can be reached.
- [x] The lines between tiles drawn and dragged.
- [ ] A line that stops where a window says it can be no smaller, and a
      part of the theme for the line.
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
- [ ] Choosing a window for an empty part from those on the desktop,
      shown small (ADR 38).
- [ ] A window that nobody asked for, opening while a fullscreen
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
- [ ] A clipboard that outlives the program that copied: a manager of
      our own with the `clipboard` role, which would also let
      `omorfos-screenshot` stop at once. Today it stays running to give
      its picture when pasted.
- [x] The `private` rule, which leaves a window blank in any picture
      taken of the screen, for as long as it matches (ADR 38).
- [x] Pictures of one window (`ext-foreign-toplevel-image-capture-source`),
      and the screenshot tool taking one of the window with focus, with
      `--window` and `Alt+Print`.
- [ ] Known gaps in pictures of a window: one larger than its screen is
      cut to the screen; what is see-through in it is black; and it is
      given without the frame omorfos draws, with no way to ask for it.
- [ ] A private window blurred in pictures in place of blank, as a
      choice of the rule (`private: blur`), once glass has brought the
      blur and a way to draw a window by itself. A blur hides less than
      a blank does, which the guide is to say.
- [ ] Known gaps in what is private: a notification about a private
      window, its menus if it is written for X11, and its last moment as
      it fades when closed, are in a picture; and a program cannot yet
      say of its own window that it is private.
- [ ] The stream that sharing a screen in a browser or a call needs
      (PipeWire), in buffers of the GPU so that nothing is copied, with
      the user asked each time.
- [ ] Known gaps in pictures of the screen: a screen that is turned is
      given as it is stored, not as it is seen; a pointer an application
      draws itself is given by itself as the ordinary one; the pointer's
      look is not worked out again when what is under it goes, only when
      it next moves; and the mark sits over the end of a panel at the top.
- [x] The portals answered by a program of our own,
      `xdg-desktop-portal-omorfos`, started with the session: so far
      the settings that tell applications light from dark and the
      accent, read from the theme and told again when it changes.
- [x] The Screenshot portal, with the user asked each time by a
      question of the backend's own, over everything, which is not in the
      picture.
- [ ] Known gaps in the Screenshot portal: the colour picker it also
      offers is not answered; the user cannot choose a screen, a window or
      a region, only all of it; an application is named by its
      identifier, not by the name it shows; and the question borrows the
      launcher's look in place of a part of its own.
- [x] Sharing the screen through the portal, as a PipeWire stream, with
      the user asked each time, and the mark on the screen while it is
      shared. The compositor puts each picture straight into memory the
      reader maps, nothing copied on the way; only while something reads
      the stream, only as the screen changes, and no more than sixty times
      a second. A reader that comes anew, as a call's preview and then the
      call, is shown the screen at once, and sharing carries on through
      the session being locked, showing nothing of it.
- [ ] Known gaps in sharing the screen: the first screen is shared, with
      no choice of screen, window or region; a screen that changes size
      ends the sharing; the size the application is told is in pixels,
      not in units of layout; a choice cannot be remembered for next time;
      the user cannot stop sharing from the desktop, only from the
      application; and the compositor still reads each picture back from
      the GPU and turns its colours on the CPU, where buffers of the GPU
      handed to the reader would copy nothing at all (next).
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
- [ ] A lock screen of our own, `omorfos-lock`, in place of relying on
      another's: started by a key, by being away, by the lid and by
      logind. It is the most security-critical program here after the
      compositor and is written and reviewed as such.
- [ ] Sleep, restart and shut down, by logind, and what the lid and the
      power button do.
- [ ] Screens arranged by the user: above and below as well as side by
      side, resolution, refresh rate, rotation, mirroring, one made
      primary, and arrangements remembered for the screens that are
      plugged in.
- [ ] Wallpaper (wish 6): a colour or a picture per screen, filled,
      fitted, stretched, centred or tiled; a directory or several shown
      in turn, in order or shuffled, at an interval. A program of its
      own, which the theme's `desktop` gives way to.
- [ ] Keys for volume, brightness and media, with a small display of the
      level over the screen that goes by itself.
- [ ] A switcher for `Alt+Tab` (wish 5): the windows in the order they
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
- [ ] Writing in languages that need an input method
      (`text-input-v3`, `input-method-v2`), and switching keyboard
      layout by a key.
- [ ] What games and remote desktops need of the pointer: locked,
      confined, and moved without a position (`pointer-constraints`,
      `relative-pointer`).
- [ ] Gestures on the touchpad: workspaces by a swipe of three fingers.
- [ ] A polkit agent, so that a program can ask for the administrator's
      password, and `omorfos-pinentry` for keys and passphrases
      (wish 9): the screen dimmed and blurred behind one prompt, what is
      typed hidden unless asked to be shown. Both take the keyboard
      exclusively, as the lock screen does.
- [ ] Programs listed in `~/.config/autostart` started with the session,
      and the session's environment told to D-Bus and systemd, without
      which portals and much else do not find it.
- [ ] Smaller protocols that programs complain of lacking:
      `xdg-toplevel-icon`, `xdg-foreign`, `presentation-time`,
      `keyboard-shortcuts-inhibit`, content type and tearing control.

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
      files.
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
      screen.

Reaching people:

- [x] The README put right: what omorfos is now, how to build, install
      and try it, and where the guide, the roadmap and the decisions are.
- [ ] Pictures of it in the README, once it wears the design language.
- [ ] A first release, which is also the first real test of the release
      pipeline (see [RELEASING.md](RELEASING.md)).
- [ ] The shell in the user's language.
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
- **Fluxbox's features.** Frames with tabs drawn by the compositor, moving
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
