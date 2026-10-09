# 14. Idle and session lock

- Status: accepted
- Date: 2026-10-09

## Context

A desktop that is logged in to has to be able to lock, and to know when
its user has gone away so that something can lock it. On Wayland both are
jobs for separate programs: an idle daemon is told when there has been no
input for a while, and a screen locker asks the compositor to lock the
session and shows something to unlock it with. The compositor's part is
small, but it is the part that makes a lock a lock. A locker that can be
looked around or typed past is worse than none, because it is trusted.

Both were put off from milestone 7: idle notification needs timers on the
event loop, which the compositor now has (ADR 9), and a lock is only worth
proving once there is a real session to lock (ADR 10, ADR 13).

## Decision

**ext-idle-notify, with idle-inhibit.** Every input event counts as the
user being present. A surface can ask for the session not to go idle, and
is heeded only while its window or panel is actually on screen: one on
another workspace, minimized, or behind a lock has no say.

**ext-session-lock, enforced in three places and nowhere else.** While the
compositor holds a lock:

- *Drawing* draws the locker's surface on black and returns. It does not
  look at windows, panels, the menu or the notice, so nothing has to be
  remembered to be hidden.
- *Input* is handed, whole, to a separate path that knows only the lock
  surface. Key bindings, the menu, drags and windows are not consulted, so
  nothing has to be remembered to be disabled. Switching virtual terminal
  still works: it leads to a login prompt, not into the session.
- *The keyboard* belongs to the lock surface or to nothing, decided before
  anything else that might want it.

A feature added later is locked out by default, because it is added below
these points, not beside them.

**The session is locked from the moment it is asked.** The keyboard and
pointer leave the application at once. The locker is told the session is
locked only after a frame without the session in it has been drawn and
shown, as the protocol requires, so that it does not let the machine sleep
with the desktop still on the screen.

**Only the holder's surface is shown.** A second locker is refused while
the first lives. Being refused still leaves it with an object it can ask
for a lock surface through; such a surface is ignored, so it can neither
cover the real prompt nor collect what is typed at it.

**A dead locker leaves the session locked,** showing black and accepting
nothing. A new locker may then take over. This is what the protocol asks
for and the only safe answer: the alternative is that crashing the locker
unlocks the screen.

**Any client may lock.** Locking reveals nothing and the user can undo it,
so there is no list of trusted lockers to maintain.

## Consequences

- If the locker dies there is no way back in from the keyboard. Recovery
  is from another virtual terminal, by starting a locker on the session's
  display so that it can be unlocked properly.
- Programs of the user's that talk to the IPC socket can still act on a
  locked session. They run as the user and could do anything anyway; the
  lock is against whoever is at the keyboard.
- The compositor does not start a locker or an idle daemon by itself, or
  lock before sleep. Those are the daemon's to arrange; `autostart` in
  `config.yaml` can start it.
- Turning the screens off was first left to a daemon too, which would have
  needed an output power protocol that Smithay does not provide. It is
  instead a setting, `idle.screen_off`: the compositor already knows when
  there was last input and what is asking to stay awake, and says through
  `Compositor::screens_on` whether the screens should be on. The decision
  is tested with the clock the tests control; a backend that drives
  screens acts on it. It is off unless a time is given, because the part
  that powers a screen down and up again is hardware glue that cannot be
  tested here.
- Nothing is configured: which locker to use and after how long are the
  idle daemon's settings.
- A key handled between the request to lock and the next sync can be
  tested only by driving the pieces separately, which the harness cannot
  do. The keyboard is taken away in the handler itself for that reason.
