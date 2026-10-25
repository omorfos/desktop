# 58. Passphrases asked in the sealed prompt, in our words only

- Status: accepted
- Date: 2026-10-25

## Context

GnuPG asks for the passphrase of a key through a pinentry: a program
`gpg-agent` starts as its own, and speaks to on its standard input and
output (the Assuan protocol). SSH keys that `gpg-agent` holds are asked
for the same way. ADR 35 gave this to `omorfos-ask`, and ADR 48 left
`omorfos-pinentry` to use the sealed prompt, the one surface no program
but ours can draw.

What makes that prompt safe for polkit is where its words come from:
polkit's actions are described by policy files the system installs. A
pinentry's are not. `gpg-agent` passes on whatever description it is
given (`SETDESC`), and any program the user runs can ask it for a
passphrase with words of its own (`gpg-connect-agent GET_PASSPHRASE`),
and be given what is typed. Shown in the sealed prompt, those words would
have the desktop's own surface say "Type your login password to go on",
and the answer go to whoever wrote it.

What is advised elsewhere says the same:

- NIST SP 800-53 SC-11 and the Common Criteria's `FTP_TRP` ask that a
  trusted path be set going only by the user or the system's security
  functions, and that what is typed through it is kept from programs that
  are not trusted.
- Microsoft keeps UAC's prompts on a desktop only the system can draw on,
  and says plainly that a prompt for credentials that can be imitated is
  the one that gives them away.
- The NCSC asks that defences against phishing not rest on users noticing
  it, as no training teaches anyone to spot every attempt.
- GNOME's pinentry shows GnuPG's description in its system prompt, which
  dims the screen as ours does. That is the choice this decision does not
  make.

## Decision

**The sealed prompt asks for a passphrase in words of omorfos's alone.**
Nothing a program writes reaches it: not GnuPG's description, title,
prompt, the text of its buttons, or its error. The prompt says that
GnuPG asks for the passphrase of a key, or for a new passphrase twice, in
a heading of ours that cannot be changed. Asked to confirm something,
which `gpg-agent` asks in words that are a program's, the pinentry says
no for now; a message, which asks nothing, it says it has shown.

**Which key is said from what can be checked.** GnuPG names the key by its
keygrip (`SETKEYINFO`), a number derived from the key itself. The
pinentry looks the keygrip up with GnuPG, in the same home as the agent
that asked, and shows the key's fingerprint and its first user ID, as
GnuPG lists them; for a key held for SSH, its SSH fingerprint. A keygrip
that cannot be looked up, or none at all, is shown as "a key GnuPG does
not name". A user ID is a key's owner's own words, and is shown as a
name, set apart, never as a sentence of the prompt's.

**What GnuPG says is kept in the log, not shown.** Its description is
passed on with the request, and `omorfos-ask` writes it to the session's
journal, so that its exact words can be read when they are wanted. A wrong passphrase is said in our words
("That passphrase was not right"), as GnuPG asks again.

**`omorfos-pinentry` asks `omorfos-ask`, which shows the prompt.** It is
started by `gpg-agent`, not by the compositor, so it cannot have the `ask`
role. It speaks Assuan to `gpg-agent`, and asks `omorfos-ask` over a
socket in the runtime directory that only the user can open, which
`omorfos-ask` listens on. `omorfos-ask` takes a request only from a
process whose program is the installed `omorfos-pinentry`, as the kernel
says (`SO_PEERCRED`, `/proc/<pid>/exe`), and whose parent is `gpg-agent`.
The parent is known by the name the kernel keeps of it, as `gpg-agent`
keeps what it runs from being read, to keep what it holds from being
looked into; and a process may give itself that name. So the check
raises the bar, and is not the boundary. A program of the user's can start a `gpg-agent` of its own,
with a key of its own made to look like the user's, and ask through it.
What it can then be given is a passphrase the user typed believing it a
GnuPG key's, never one the prompt asked for as anything else.

**What is typed is handed back and wiped**, as in ADR 48: given to
`gpg-agent` as the protocol has it, and the field's bytes wiped when the
prompt goes. One request at a time, after polkit's or before it.

**Where there is no sealed prompt**, outside an omorfos session or with
`omorfos-ask` not running, the pinentry says so to `gpg-agent` as an error
and asks nothing: GnuPG then says why, and another pinentry can be named.

## Alternatives

- **GnuPG's words, set apart in the prompt**, as GNOME does. More is said
  of what wants the key, but it rests on the user telling our words from
  GnuPG's every time, which the NCSC advises against, and gives a program
  the trusted surface to write on.
- **An ordinary window**, as `pinentry-gtk` and `pinentry-qt` are. It lends
  no trust, so there is nothing to borrow, but anything can draw one, and
  a prompt for credentials that can be imitated is the classic case of
  giving them away.
- **The pinentry given the `ask` role** and drawing the prompt itself. The
  compositor gives roles to what it starts or the configuration names;
  `gpg-agent` starts the pinentry, and a role that came with a program's
  name would be anyone's who named theirs so.

## Consequences

- GnuPG's own explanation of why a key is wanted, such as which file is
  being decrypted, is not on the screen. Which key is.
- `gpg-agent.conf` names `omorfos-pinentry` as its `pinentry-program`; the
  guide says how, and nothing changes until it does.
- SSH keys held by `ssh-agent` rather than `gpg-agent` ask through
  `SSH_ASKPASS`, whose prompt is a program's words too; that is left for
  now, and is to be done the same way if it is done.
- `omorfos-ask` gains a socket and a second kind of request, and keeps
  polkit's as they are.
