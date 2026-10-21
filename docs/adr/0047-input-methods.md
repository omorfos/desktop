# 47. Input methods

- Status: accepted
- Date: 2026-10-21

## Context

Chinese, Japanese, Korean and many other languages have more characters
than a keyboard has keys. They are typed through an input method: a
program that takes the keys, works out the characters meant, shows those
it could mean beside the text, and puts the one chosen in the field.
Others are written with letters of their own, and need only the keyboard
to type with another layout.

On Wayland the field says it is being typed into, and where its text is
(`text-input-v3`); the input method is told, and says what to put there
(`input-method-v2`); while it types it may take every key, and pass on
the keys it has no use for through a keyboard of its own
(`virtual-keyboard-v1`). `fcitx5` and `ibus` speak these, and GTK, Qt and
the browsers speak the first.

## Decision

**The input method is a program with the `input` role.** It is shown
what is typed into whatever has the keyboard, passwords among it, and
what it types goes wherever the keyboard is: it is a keylogger that the
user chose. ADR 37 named the role for it, and given by the user is the
only way it is had. Without the role a program is not shown that input
methods or keyboards of one's own exist. Fields are anyone's: a field
saying it is being typed into tells nobody anything until there is an
input method.

**Its popup is the compositor's to place.** The popup is a surface the
input method gives, and the compositor shows it under the text it is
for, as the field says where that is, over everything but the pointer,
since the field may be in anything; above the text where there is no
room under it, and within the screen. Where the field's window is, is
worked out each time it is drawn, so that the popup goes with a window
that moves. Text in no window or panel that is shown has no popup.

**A locked session is typed into without it.** While the session is
locked, any grab of the keyboard, which is only ever the input method's,
is let go of before a key is handled: what is typed then is the user's
secret, and goes to the locker alone. The input method grabs again when
next it is asked to type. Keys it types as a keyboard still go to the
locker, as typing as the user is what the role is.

**Switching layout is an action, `next_layout`, bound to no key.** The
layouts are those `keyboard.layout` lists, as XKB has always had them.
Bindings go by the first layout that writes in Latin letters, as Smithay
gives them, so that a key bound in one is the same key in another. Which
key, if any, is to switch by default is left for the review of what is
bound by default, with the look; XKB's own options for it still work.

## Alternatives

- **An input method of our own, or ibus or fcitx built in.** Each
  language's method is a large thing with dictionaries of its own, and
  people have the one they use. The compositor's part is the protocols.
- **No role: any program may be the input method**, as on most Wayland
  desktops. Then any program running could read every password typed.
- **The popup placed by the input method.** It cannot know where the
  text is on the screen, only on the field.

## Consequences

- One input method at a time. Smithay, given a second, tells the first
  that it may no longer be one rather than telling the second, and
  forgets the first when a second goes; started again after it stops, an
  input method works. The user starts one.
- Programs written for X11 type through their own input method, as
  before; the X server does not speak these protocols.
- An input method that puts itself in the background as it starts lets
  go of the connection that has the role, and so must be started without
  doing so.
