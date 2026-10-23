# 52. The lock screen asks PAM, through `nonstick`, in a helper for each way of unlocking

- Status: accepted
- Date: 2026-10-23
- Amends: [14. Idle and session lock](0014-idle-and-session-lock.md),
  [35. Where omorfos meets the rest of the system](0035-meeting-the-rest-of-the-system.md)

## Context

The session is locked by whatever program `locker` names, `swaylock` most
often, which the compositor starts with the role to lock (ADR 37). ADR 35
gave omorfos a lock screen of its own, `omorfos-lock`, that asks PAM, so
that whatever the system knows its user by unlocks: a password, a finger,
a security key. After the compositor it is the program here that most
needs to be right: a mistake in it either lets anyone in, or keeps the
user out of their own session.

Three things were to be decided: how PAM is reached without `unsafe` of
our own, how ways of unlocking other than a password are offered, and
what happens when something goes wrong.

## Decision

### PAM through `nonstick`, pinned and read

PAM is a C library, and no `unsafe` is written here (ADR 40), so it is
reached through a crate. Of those whose licence `deny.toml` allows, two
were read in full where it matters, the conversation through which the
secret passes:

- **`pam` 0.8** (MIT or Apache-2.0), the most used, was turned down. Its
  conversation callback is marked `FIXME: verify this`. It ends the whole
  conversation when PAM sends an error message, which PAM does while it
  goes on: a fingerprint module says "Failed to match fingerprint" that
  way, so one wrong finger would end the try. When it ends a conversation
  so, it frees the array of answers without the answers in it, leaking
  copies of the secret, and nothing it holds of the secret is ever
  cleared. It brings `bindgen` 0.69 and `syn` 1 besides the versions
  already in the tree.
- **`nonstick` 0.1.2** (MIT) was taken. Its callback checks every pointer
  it is given, turns every failure into PAM's error code rather than a
  panic across the C boundary, passes error and information messages on
  without ending anything, knows every kind of message Linux-PAM sends,
  and clears every string it frees with volatile writes. It depends on
  `libc`, `bitflags` and three small crates of its own, all MIT, and no
  generator of bindings.

It is pinned to the exact version (`=0.1.2`), and every update is read
before it is taken, as the `dependencies` routine says for any crate.

### Each way of unlocking is a PAM service, asked in a process of its own

What can unlock is what PAM is set up to accept, and nothing in omorfos
knows of fingerprints or keys: `pam_fprintd` reads a finger, `pam_u2f` a
FIDO2 or U2F key (a YubiKey among them), `pam_yubico` a YubiKey's one-time
code, `pam_pkcs11` or `pam_sss` a smart card. But PAM asks the modules of
one service one after another, so a service that tried a finger before a
password would make the user wait for the finger to time out before they
could type.

So, as GDM does, each way is a service of its own, and they are asked at
once, each in a process of its own:

| Way | Service | Shipped as |
|---|---|---|
| `password` | `omorfos-lock` | `auth include login`, as `swaylock`'s is |
| `fingerprint` | `omorfos-lock-fingerprint` | `auth required pam_fprintd.so` |
| `key` | `omorfos-lock-key` | `auth required pam_u2f.so cue` |

`lock.fingerprint` and `lock.key` in `config.yaml` say whether those two
are offered, as they are unless told otherwise. The password always is,
and cannot be turned off: a list of ways that could leave it out would let
a reader that breaks, or a key that is lost, keep the user out of their
own session. Whichever says the user is who they say first unlocks the
session, and the others are stopped. A way whose module is not installed,
or for which the user has nothing enrolled, says so at once and is left
off the screen without a word.

The password is the only way the user types into. Another that asks for
something to be typed, as `pam_u2f` does for a key's PIN, is stopped and
said to be one the lock screen does not ask for there; a PIN-protected
key goes in the password's service instead, where its questions come in
turn.

**A process for each, not a thread,** because PAM does not promise that
two transactions may run at once in one process, and modules hold state
of their own; and so that a module that crashes or hangs takes its own
process with it, not the lock screen. The process is `omorfos-lock`
itself, started again with `check` and the service. It speaks the line
protocol polkit's helper speaks, which `omorfos-ask` already reads: what
PAM asks or says, a line each, and `SUCCESS` or `FAILURE`; what is typed
is written back a line for each question. It asks `pam_authenticate`
only, as `swaylock` does: `pam_acct_mgmt` would keep a user whose
password has just expired out of the session they are already in.

### What is shown, and when

Until a key or a mouse button is pressed, only the time is shown, in the
middle. Then it moves up, and under it come, rising into place and out of
nothing as the theme's `question` transition has things move, the user's
face and name, the field, and what each way says. Left alone for 30
seconds, the screen shows only the time again and the field is emptied, so
that half a password is not left waiting. The ways are asked all along: a
finger unlocks without a key pressed first, and a finger that does not
match shows the rest, to say so.

- **The key that wakes the screen types nothing.** One pressed to see the
  screen, or to wake it from dark, would otherwise start the password with
  a stray character, which shows only as one dot too many. Whoever starts
  typing at once loses the first character, and finds out from a wrong
  password; the stray key is the commoner case.
- **The name** is the user's full name, the first part of the GECOS field
  of `/etc/passwd` before any comma, as other login screens show it, or
  else the name they log in with. **The face** is the first of
  AccountsService's icon for the user, `~/.face` and `~/.face.icon` that
  reads as a PNG or a JPEG, read by the wallpaper's reader, which treats a
  file as untrusted, and cut to a circle. Without one, the user's
  initials are drawn in a disc as the prompt for a secret looks, so that
  the screen is not bare where a face would be. The name is not limited to
  a number of characters, which says nothing of how wide it is drawn: it
  is shown whole, measured at the scale it is drawn at, and cut short only
  where it is wider than the screen less its margins.
- **A way other than the password is shown only once it has said
  something,** and no longer than the try it said it in. `pam_u2f` fails
  at once and says nothing when no key is plugged in, and says its `cue`
  only once it has found one, so a key is shown only while it is plugged
  in. `pam_fprintd` asks for a finger once it has a reader. So a screen
  left locked does not tell whoever is at it what else would unlock it.
  No device is looked for by the lock screen itself: what the module says
  is what is shown.

### When something goes wrong

- **The lock screen ends without unlocking:** the session stays locked
  (ADR 14), and the compositor starts the locker again, at most three
  times in a minute; after that the screen stays black and locked, and
  the user can unlock from another virtual terminal with
  `omorfosctl action lock`, which starts a new locker that takes over.
- **A helper ends without an answer:** that way failed; the others go
  on, and the password can be tried again.
- **A secret that holds a NUL byte** is never handed to PAM: what is typed
  is text from the keyboard and holds none, and what reaches the helper
  is cut at the first, since `nonstick` panics on one inside a string it
  is to give PAM.

## Risks

Kept here so that they are looked at again when any of this is reviewed.

- **`nonstick` is young and has one maintainer.** 0.1.x may change its API
  at any release, fixes may be slow to come, and fewer eyes have been on
  it than on `pam`. Mitigations: the exact pin; reading every update; the
  helper's use of it is one small file, so that moving to another crate
  is a small change. To be looked at again if `pam` is fixed, or another
  wrapper under a licence `deny.toml` allows matures.
- **`nonstick` carries code we do not use:** the half for writing PAM
  modules, and a stray `handle.rs.orig` in its package. Neither is
  reached from the helper; both are more to read at each update.
- **`nonstick` can panic** on a NUL inside a string it hands PAM, and in
  two `unwrap`s in its environment code, which the helper does not use.
  The NUL is stripped first. A panic in the helper ends only that way's
  try.
- **Any one way unlocks.** Ways asked at once mean the session is as hard
  to get into as the weakest of them: a fingerprint can be lifted, a key
  taken. That is the user's choice, made with `lock.fingerprint` and
  `lock.key`; two factors
  together are had by putting both in one service, `omorfos-lock`, and
  the guide says so, so that nobody thinks they have two by listing two.
- **Each service counts failures for itself.** `pam_faillock`, where it
  is used, may lock the user out of one way and not another, or count a
  wrong finger against the password; how is the system's PAM set-up's.
- **What is typed lives in the lock screen's memory** as an ordinary
  string until it is cleared, and passes through a pipe to the helper,
  which is the same user's. It is not cleared with volatile writes;
  `zeroize` would, and is the next thing to weigh if this is tightened.
- **PAM modules run as the user,** in the helper, and `pam_unix` checks the
  password through `unix_chkpwd`, which is set-uid, as it does for every
  locker. A module that misbehaves can do whatever the user can.
- **What is tested is the program, not the system's set-up.** The tests
  use `pam_wrapper`, which loads services from a directory of the test's
  own, with `pam_matrix` holding made-up users and `pam_chatty` saying
  what PAM would; the real modules, `pam_unix`, `pam_fprintd` and
  `pam_u2f`, are only tried on real hardware.
- **The lock screen says who it is for.** The full name and the face are
  shown to whoever is at the screen, as the login screen shows them. A user
  who would rather not has them kept from it: an empty GECOS field shows
  the login name, and no face file shows initials.
- **The face is a file the user, or anyone who can write their home, puts
  there.** It is read with the limits the wallpaper's pictures are: no
  larger than 256 MiB, no more than 16384 pixels along a side, and the
  decoders of `png` and `zune-jpeg`, which are safe Rust. One that cannot
  be read is passed over.
- **The service files are the system's once installed.** A distribution or
  an administrator may change them, for better or worse, and should; the
  packages ship them as configuration files.

## Consequences

- A new crate, `omorfos-lock`, a new dependency (`nonstick`), and three
  PAM service files in the packages. Building needs PAM's development
  files (`libpam0g-dev`), and the tests need `pam_wrapper`
  (`libpam-wrapper`); CI installs both.
- `locker` still names the locker. Until `omorfos-lock` has been used for
  a while, it is not the default; `locker: [omorfos-lock]` chooses it.
- The on-screen keyboard (stage 7) is to be usable on the lock screen.
