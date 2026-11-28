# `keys.yaml`

```yaml
bindings:
  - keys: Super+Return     # in place of the built-in terminal
    action: { spawn: [foot] }
  - keys: Super+l
    action: lock
  - keys: Super+x t        # a chain: Super+x, then t
    action: { workspace: 2 }
unbind: [Super+h]          # a built-in binding done without
```

The bindings you write are made beside the built-in ones, which go on
working on every key you leave alone. One you write on keys a built-in
binding uses takes its place, and so does a chain that begins with them,
as `Super+Return t` would. `unbind` lists built-in bindings to do without,
by their keys; one that names no built-in binding is a mistake, and said
to be. The built-in bindings are a Mac's, with Super for Command; to have
the ordinary way's, as Windows, GNOME and KDE have them, say `defaults:
ordinary` (see [The ordinary way](#the-ordinary-way)); to have none of
them and only your own, say `defaults: false`.

A chord is zero or more modifiers and a key, joined by `+`. Modifiers are
`Ctrl`, `Alt`, `Shift` and `Super` (case does not matter; `Control`, `Mod1`
and `Mod4` also work). Keys are XKB keysym names such as `Return`, `a`, `F1`
or `XF86AudioMute`. A binding names the key, not the symbol it types: write
`Super+Shift+q`, not `Super+Q`. A binding whose key name does not exist is
ignored and reported.

Two of your bindings may not use the same keys, and a chain of yours may
not begin with keys you have bound on their own.

## Super

Super is the desktop's: a program never sees a key pressed while it is
held. A chord with Super that is bound does what it is bound to; any
other reaches the program with the keyboard as the same chord with Ctrl
in its place, as Cmd does on macOS: `Super+c` copies, `Super+v` pastes,
`Super+z` undoes, `Super+f` finds, `Super+Shift+t` is `Ctrl+Shift+t`. In
a terminal, where `Ctrl+c` interrupts, it is Ctrl and Shift together,
which terminals copy, paste and open tabs with. Super with the arrows
goes to the ends of the line and of the text, as Cmd does: `Super+Left`
reaches the program as `Home`, `Super+Right` as `End`, `Super+Up` as
`Ctrl+Home` and `Super+Down` as `Ctrl+End`, and with Shift chooses as it
goes; in a terminal too, without Shift added. Which programs are
terminals is `keyboard.terminals` in `config.yaml`, by their `app_id`,
where `*` stands for any run of characters; `keyboard.super_as_ctrl:
false`, or the ordinary way, has a chord with Super that nothing binds
reach the program as it is. A chord you bind with Super is no longer the
program's: `Super+s` bound is no longer save anywhere.

The shell's own fields, in the launcher, the boxes that ask for a
password and the lock screen, are given Super as it is, and read it as a
Mac's read Command: `Super+c`, `x`, `v`, `a` and `z` copy, cut, paste,
choose all and undo, and Ctrl with those letters does nothing there;
`Super` with the arrows goes to the ends of what is typed, and
`Super+BackSpace` takes out all before the cursor. In the ordinary way
they read Ctrl instead, as everything else does. Super with a digit,
while the launcher is open, goes to its modes, not to a workspace
([Modes](launcher.md#modes)).

A program that needs every key, as a virtual machine, a remote desktop
or a game may, can ask for them (`keyboard-shortcuts-inhibit`), and is
given them while its window has the keyboard, the desktop's bindings and
Super among them; a notice says so. `Super+Escape` always takes them
back, until the program asks again.

## The ordinary way

With `defaults: ordinary`, the built-in bindings are those of the way
Windows, GNOME and KDE have keys, with Ctrl and Alt, and a chord with
Super that nothing binds reaches the program as it is:

| Keys | What they do |
|---|---|
| `Alt+Tab` | Switch windows; `Alt+Shift+Tab` back |
| `Alt+F4` | Close the window |
| `Alt+F10` | Maximize the window, or restore it |
| `Super+Left` | Have the window take the left half; `Super+Right` the right |
| `Super+Page_Down` | Go to the next workspace; `Super+Page_Up` the one before |
| `Super+l` | Lock the screen |
| `Ctrl+Alt+Delete` | Log out, asking first |
| `Super+space` | Open the launcher; `Super+Up` the overview |

and the `Print` keys, and those for the volume, the brightness and what
is playing. Super holds nothing else: `Super+Tab`, `Super+1` or
`Super+Return` reach the program unless you bind them in `keys.yaml`.

## Built-in bindings

What the keys do before you write anything, each action as `keys.yaml`
writes it; `man 5 omorfos-keys` lists them too. An action not among them,
as `overview` or `lock`, has no key until you bind one.

<!-- The built-in bindings, made by a test: OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page -->
| Keys | Action | What it does |
|---|---|---|
| `Super+Return` | `{ open: terminal }` | Open the default application of KIND |
| `Super+space` | `show_launcher` | Open the launcher |
| `Super+q` | `close_program` | Ask the focused program's windows to close |
| `Super+m` | `minimize` | Hide the focused window |
| `Super+h` | `hide_program` | Hide the focused program's windows |
| `Alt+Super+h` | `hide_others` | Hide every other program's windows |
| `Ctrl+Super+f` | `fullscreen` | Make the focused window fullscreen, or not |
| `Alt+Super+Up` | `maximize` | Maximize the focused window, or restore it |
| `Alt+Super+Left` | `{ tile: left }` | Tile the focused window to that half |
| `Alt+Super+Right` | `{ tile: right }` | Tile the focused window to that half |
| `Super+Tab` | `switch_programs` | Switch programs, or go on to the next |
| `Shift+Super+Tab` | `switch_programs_back` | The same, going back |
| `Super+grave` | `next_window` | Focus the focused program's next window |
| `Shift+Super+grave` | `previous_window` | Focus the focused program's window before |
| `Ctrl+Super+Tab` | `next_tab` | Show the next tab of the focused window |
| `Ctrl+Shift+Super+Tab` | `previous_tab` | Show the previous tab of the focused window |
| `Ctrl+Super+Up` | `overview` | Open the overview of the workspace, or close it |
| `Ctrl+Super+Down` | `show_program_windows` | Show the focused program's windows alone |
| `Ctrl+Super+Right` | `next_workspace` | Show the next workspace |
| `Ctrl+Super+Left` | `previous_workspace` | Show the workspace before |
| `Ctrl+Super+n` | `new_workspace` | Make a workspace after the last, and show it |
| `Ctrl+Shift+Super+n` | `move_to_new_workspace` | Send the focused window to a new workspace |
| `Ctrl+Super+q` | `lock` | Lock the session |
| `XF86ScreenSaver` | `lock` | Lock the session |
| `Shift+Super+q` | `log_out` | End the session, once the user says so |
| `Print` | `{ spawn: { run: [omorfos-screenshot], may: [capture, clipboard] } }` | Start a program, with no shell |
| `Shift+Print` | `{ spawn: { run: [omorfos-screenshot, --pick, --to, clipboard], may: [capture, clipboard, panel, overlay] } }` | Start a program, with no shell |
| `Alt+Print` | `{ spawn: { run: [omorfos-screenshot, --window], may: [capture, clipboard, panel] } }` | Start a program, with no shell |
| `Shift+Super+3` | `{ spawn: { run: [omorfos-screenshot], may: [capture, clipboard] } }` | Start a program, with no shell |
| `Shift+Super+4` | `{ spawn: { run: [omorfos-screenshot, --pick], may: [capture, clipboard, panel, overlay] } }` | Start a program, with no shell |
| `Shift+Super+5` | `{ spawn: { run: [omorfos-screenshot, --pick], may: [capture, clipboard, panel, overlay] } }` | Start a program, with no shell |
| `Ctrl+Shift+Super+3` | `{ spawn: { run: [omorfos-screenshot, --to, clipboard], may: [capture, clipboard] } }` | Start a program, with no shell |
| `Ctrl+Shift+Super+4` | `{ spawn: { run: [omorfos-screenshot, --pick, --to, clipboard], may: [capture, clipboard, panel, overlay] } }` | Start a program, with no shell |
| `XF86Display` | `next_screens` | Arrange the screens the next way |
| `Super+F1` | `next_screens` | Arrange the screens the next way |
| `XF86Keyboard` | `next_layout` | Type with the next keyboard layout |
| `XF86LaunchA` | `overview` | Open the overview of the workspace, or close it |
| `XF86AudioRaiseVolume` | `{ spawn: [omorfos-session, volume, up] }` | Start a program, with no shell |
| `XF86AudioLowerVolume` | `{ spawn: [omorfos-session, volume, down] }` | Start a program, with no shell |
| `XF86AudioMute` | `{ spawn: [omorfos-session, volume, mute] }` | Start a program, with no shell |
| `XF86AudioMicMute` | `{ spawn: [omorfos-session, microphone, mute] }` | Start a program, with no shell |
| `XF86MonBrightnessUp` | `{ spawn: [omorfos-session, brightness, up] }` | Start a program, with no shell |
| `XF86MonBrightnessDown` | `{ spawn: [omorfos-session, brightness, down] }` | Start a program, with no shell |
| `XF86AudioPlay` | `{ spawn: [omorfos-session, media, play-pause] }` | Start a program, with no shell |
| `XF86AudioPause` | `{ spawn: [omorfos-session, media, play-pause] }` | Start a program, with no shell |
| `XF86AudioNext` | `{ spawn: [omorfos-session, media, next] }` | Start a program, with no shell |
| `XF86AudioPrev` | `{ spawn: [omorfos-session, media, previous] }` | Start a program, with no shell |
| `XF86AudioStop` | `{ spawn: [omorfos-session, media, stop] }` | Start a program, with no shell |
| `Ctrl+Alt+Super+Left` | `{ focus: left }` | Focus the window beside the focused one |
| `Ctrl+Alt+Shift+Super+Left` | `{ swap: left }` | Swap the focused window with the one beside it |
| `Ctrl+Alt+Super+Right` | `{ focus: right }` | Focus the window beside the focused one |
| `Ctrl+Alt+Shift+Super+Right` | `{ swap: right }` | Swap the focused window with the one beside it |
| `Ctrl+Alt+Super+Up` | `{ focus: top }` | Focus the window beside the focused one |
| `Ctrl+Alt+Shift+Super+Up` | `{ swap: top }` | Swap the focused window with the one beside it |
| `Ctrl+Alt+Super+Down` | `{ focus: bottom }` | Focus the window beside the focused one |
| `Ctrl+Alt+Shift+Super+Down` | `{ swap: bottom }` | Swap the focused window with the one beside it |
| `Super+1` | `{ workspace: 1 }` | Show workspace N |
| `Ctrl+Super+1` | `{ move_to_workspace: 1 }` | Send the focused window to workspace N |
| `Super+2` | `{ workspace: 2 }` | Show workspace N |
| `Ctrl+Super+2` | `{ move_to_workspace: 2 }` | Send the focused window to workspace N |
| `Super+3` | `{ workspace: 3 }` | Show workspace N |
| `Ctrl+Super+3` | `{ move_to_workspace: 3 }` | Send the focused window to workspace N |
| `Super+4` | `{ workspace: 4 }` | Show workspace N |
| `Ctrl+Super+4` | `{ move_to_workspace: 4 }` | Send the focused window to workspace N |
| `Super+5` | `{ workspace: 5 }` | Show workspace N |
| `Ctrl+Super+5` | `{ move_to_workspace: 5 }` | Send the focused window to workspace N |
| `Super+6` | `{ workspace: 6 }` | Show workspace N |
| `Ctrl+Super+6` | `{ move_to_workspace: 6 }` | Send the focused window to workspace N |
| `Super+7` | `{ workspace: 7 }` | Show workspace N |
| `Ctrl+Super+7` | `{ move_to_workspace: 7 }` | Send the focused window to workspace N |
| `Super+8` | `{ workspace: 8 }` | Show workspace N |
| `Ctrl+Super+8` | `{ move_to_workspace: 8 }` | Send the focused window to workspace N |
| `Super+9` | `{ workspace: 9 }` | Show workspace N |
| `Ctrl+Super+9` | `{ move_to_workspace: 9 }` | Send the focused window to workspace N |
<!-- The end of the built-in bindings. -->

## Actions

`omorfosctl action`, run alone, lists them too, in groups, each with what
it takes and what it does.

| Action | Effect |
|---|---|
| `spawn: [program, arg, ...]` | Start a program. No shell is involved. |
| `close` | Close the focused window. |
| `close_program` | Ask every window of the focused window's program to close, as Cmd+Q quits a program on a Mac; each may ask to save first. A program is the windows that share an application's name. |
| `hide_program` | Minimize every window of the focused window's program, on any workspace, as Cmd+H. |
| `hide_others` | Minimize every window of every other program, as Cmd+Option+H. |
| `show_desktop` | Set every window of the workspace shown aside, to show the desktop, as a Mac does: each slides straight out from the middle of the screen until a sliver is left at an edge, and none has the keyboard. They slide back where they were when it is chosen again, Escape is pressed, a window or its sliver is chosen, or one opens, and as their workspace is left for another. No key is bound to it yet; four fingers spread on a touchpad carry the windows aside, and pinched together carry them back. |
| `maximize`, `minimize` | Toggle or apply that state. |
| `fullscreen` | Put the focused window alone on a workspace made for it, filling the screen, or bring it back to the desktop it came from. |
| `tile: left` | Have the focused window take the left half of where it is; also `right`, `top` and `bottom`. The next window to open there has the other half. From a desktop, where it is becomes a workspace made for it. |
| `focus: left` | Among windows that share a workspace, give focus to the one beside the focused one; also `right`, `up` and `down`. |
| `swap: left` | Among windows that share a workspace, change the focused one over with what is beside it; also `right`, `up` and `down`. |
| `shade` | Roll the focused window up into its titlebar, or let it down. |
| `keep_above` | Keep the focused window above the others, or stop. |
| `stick` | Have the focused window on every workspace of its screen, or only the one it is on. |
| `switch_windows`, `switch_windows_back` | Open the switcher, or go on to the next window in it or back. See [Switching windows](windows.md#switching-windows). |
| `switch_programs`, `switch_programs_back` | Open the switcher with each program once, by its window used last, as Cmd+Tab does, or go on to the next or back; the program chosen comes forward with all its windows on the workspace. |
| `next_window`, `previous_window` | With keys held, open the switcher on the focused window's program's windows, in the order they were last used, as Cmd+backquote does: a quick press goes back to the one used before, and held they are shown to go through. With the switcher of programs open, go through the windows of the program it has picked, in it. Asked for with no keys held, as from a script, focus the next window of the program on its workspace, or the one before, at once. |
| `overview` | Open the overview of the workspace's windows, or close it. See [The overview](windows.md#the-overview). |
| `show_program_windows` | Open the overview with only the focused program's windows on the workspace, as App Exposé shows them, or close the overview. Three fingers or four swiped down the touchpad open it too, and up close it. See [The overview](windows.md#the-overview). |
| `focus_next`, `focus_previous` | Cycle windows, without the switcher. |
| `next_tab`, `previous_tab` | Cycle tabs in the focused group. |
| `workspace: N` | Switch to workspace N (from 1) on the screen the pointer is on. One past the last is made first; further than that does nothing. |
| `new_workspace` | Make a workspace after the last on the screen the pointer is on, and switch to it. |
| `next_workspace`, `previous_workspace` | Go to the next or the previous workspace on the screen the pointer is on. They do not go round: past the last, if a window is on it, a new one is made. |
| `move_to_workspace: N` | Send the focused window to workspace N of its screen. One past the last is made first. |
| `move_to_new_workspace` | Send the focused window to a workspace made for it, after the last. |
| `join_workspace: ID` | Put the focused window beside those sharing a workspace made for windows, and show it. The `ID` is the one `omorfosctl state` gives the workspace. The titlebar's menu offers the same by name. |
| `move_to_next_output` | Send the focused window to the next screen. |
| `show_menu` | Open the root menu. |
| `lock` | Lock the session, with the program `locker` in `config.yaml` names. |
| `stay_awake` | Keep the session from locking, the screens from going off and the machine from sleeping when you are away, until it is chosen again, the session is locked by hand, or you log out. The panel says so while it lasts, and a click there ends it. The root menu offers it. |
| `stay_awake_for: N` | The same for N seconds at most, up to a day (86400). |
| `night_light` | Make the screens warm now if they are not, as warm as `night_light.temperature` says, or as they are if they are, until night light next turns them by the time of day: switched off one evening, it is on again the next. |
| `next_layout` | Type with the next of the layouts `keyboard.layout` lists, and after the last with the first. |
| `next_screens` | Have the screens stand the next way: as the configuration says, all showing what the first shows, and each alone; each kept only if Enter is pressed within 15 seconds. |
| `guess_scales` | Guess anew the scale for every screen plugged in, as this version of omorfos would, and keep that in place of the one guessed when each was first seen. |
| `guess_scale: office` | The same for one screen, by its socket's name or one from `screens`. |
| `show_launcher` | Open the launcher. |
| `show_programs` | Open the launcher onto every program, as a grid of their icons with their names. Four fingers pinched together on a touchpad do it too, unless they are bringing back windows set aside. See [Every program](launcher.md#every-program). |
| `focus_notifications` | Give the notifications the keyboard, the newest first: Tab and the arrow keys go through it and its buttons, Enter chooses, as a click does, and Escape dismisses it. No key is bound to it yet. |
| `focus_panel` | Give the panel's bar on the screen with the pointer the keyboard: Tab and the arrow keys ring what is on it, which shows its tooltip at once, Enter or space press it, as a click does, and Escape gives the keyboard back, as Enter on a workspace or a window does. A click on the bar never takes the keyboard. No key is bound to it (ADR 115). |
| `show_welcome` | Show the welcome card, which is shown by itself at the first login: a step at a time, what the keys that matter do, light or dark, the wallpapers that come with omorfos, and the applications that open what. The root menu offers it. |
| `about` | Show "About omorfos" in the middle of the screen the pointer is on: its version and licence; what the computer is, its processor, memory, graphics cards, disks, system and kernel, and screens, as they were when it opened, with a Copy button that puts that on the clipboard as text, for a bug report, and turns green with a tick for a moment once it has, and which Enter presses too; and what it is made with that is someone else's, as `CREDITS.md` lists it. Escape, a click beside it, its button that closes on its top left corner, as the overview's has, which fades in while the pointer is on it, or `about` again puts it away, as a faint line at its foot says. On a screen too short for all of it, what it lists scrolls, by the wheel or the touchpad, the arrows, Page Up and Page Down, space, Home and End. While it is shown it has the keyboard: no window sees a key, though a binding still does what it is bound to. It is glass, as the theme's `about` has it. The root menu offers it. |
| `reload_config` | Re-read the configuration. |
| `restart_compositor` | Start the compositor again without logging out, with the newest version installed. Programs stay open through it, their windows where and as they were, those in a sandbox and those started with a role among them, as do the panel, the wallpaper, the notifications and the portal, but for programs written for X11, which close with their X server; the session stays locked if it was, programs written for X11 started again find their X server under the same number, and what starts with the session is not started a second time. Only where omorfos is the whole session, under `omorfos-hold`; elsewhere it is refused, with a notice. |
| `quit` | End the session. The programs started in it are asked to close first, as any desktop asks them, and given two seconds to; asked again meanwhile, it ends at once. |

To run a shell command, spawn the shell: `spawn: [sh, -c, "grim - | wl-copy"]`.
