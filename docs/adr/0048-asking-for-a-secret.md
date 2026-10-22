# 48. Asking for a secret

- Status: accepted
- Date: 2026-10-21

## Context

A program that wants to do what needs the administrator, installing
software or setting the clock, asks polkit, which asks the session's
authentication agent to ask the user. Without an agent the program fails
without saying why. ADR 35 gave asking to `omorfos-ask`, and ADR 37 said
the prompt is sealed: the rest of the screen dimmed and blurred by the
compositor, which no program can do to the screen, and the keyboard the
prompt's alone, so that a window drawing a password box cannot pass for
it.

## Decision

**The sealed prompt is a surface on the overlay layer of a program with
the `ask` role.** While one is up, the compositor dims every screen
behind it with the theme's `sealed` part and draws the prompt over the
dim; gives it the keyboard, past every binding, with any input method's
grab let go of, as while the session is locked; points the pointer at it
and at nothing else; and closes the switcher, the menu and any drag,
which would not see the keys or the button that end them. The capture
mark is still over it. A lock is over everything, the prompt too. The
role is given to `omorfos-ask` by the compositor, and to a program the
configuration names, as every role is: what makes the prompt the
desktop's own is that no other program can have it.

**Dimmed now, blurred with Glass.** The blur ADR 33 decided on is not
built yet. The dim alone already does what the seal is for, as nothing
else can dim the screen; the blur is added when Glass brings it.

**`omorfos-ask` is polkit's agent.** It registers with polkit for the
session (`XDG_SESSION_ID`), and is asked by polkit alone: a call from any
other program is refused, so that no other program can put the prompt up.
Of those polkit says may say yes, users and the members of groups, it
asks the user at the keyboard if they are one, and the first otherwise.
It never checks the password: polkit's own helper,
`polkit-agent-helper-1`, which is setuid, is started for each try, told
the request, and asks PAM; what PAM asks is shown, what is typed is
handed to the helper, and the helper says whether that was right. A wrong
password is asked for again, afresh, until the user gives up. Each try
has a channel of its own, so that what a helper given up on says last is
heard by nobody.

**The box says what went wrong as a head does.** A password that was
not right shakes it, three times and less each time, for no longer than
anything moves (ADR 32); a theme turns movement off as it turns colours'
changes off, by `transition: none` on the `question` part. The eye at the
end of the field shows what is typed or hides it, and says which when the
pointer rests on it. It is drawn, not taken from an icon theme, so that it
is the same everywhere.

**What is typed is not kept.** It is handed to the helper as it is
written and the bytes wiped, and the field is wiped when the prompt goes.
Copies the allocator left behind as the field grew cannot be promised
gone; nothing reads them, and the program holds no other secret.

## Alternatives

- **Another desktop's agent**, `polkit-gnome` or `lxpolkit`: what users
  of omorfos run today. Each is a window like any other, which a program
  can imitate, and looks like another desktop.
- **The compositor as the agent.** It would put the prompt up itself and
  need no role, but it would speak D-Bus, which ADR 35 keeps it from.
- **A prompt that is a window.** Anything can draw a window.

## Consequences

- `omorfos-ask` is started on hardware with the session, as the portal
  is; nested, the outer desktop's agent asks.
- The compositor gains the `ask` role and the `sealed` and
  `question-input` parts of the theme.
- One request at a time; another waits until the first is answered or
  given up on, and polkit may give up on either meanwhile.
- `omorfos-pinentry`, for GnuPG and SSH passphrases, is to use the same
  prompt.
