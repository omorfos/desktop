---
name: review
description: How omorfos is reviewed - one or more dimensions (security, architecture, components, performance, reliability, experience, tests, docs) each read by a reviewer agent of its own, at once, over a diff, a crate or the whole tree; findings checked, written up in docs/reviews/ and put on the roadmap. Use when asked for a review of any kind, before a stage is closed, or after a large change.
---

# Reviewing

A review reads; it changes no code. What it finds is written up so that
each finding can be taken up on its own, as
`docs/reviews/2026-11-15-what-a-client-can-cost.md` is, and the fixes
are changes of their own, through `handover`.

## 1. What is asked

Settle, from what was asked, and say back in a line:

- **the dimensions**: one or more of those below, `all` for every one;
- **the scope**: a diff (`main..HEAD`, or since a commit), one or more
  crates, or the whole tree;
- **the question**, if there is one beyond the dimension's own: "what a
  client can make the compositor wait for" is narrower than "security".

| Dimension | Agent | Looks for |
|---|---|---|
| security | `review-security` | what an untrusted program, IPC peer or config file can make happen: waiting, allocating, ending, more than was meant; and what is private reaching what it should not |
| architecture | `review-architecture` | dependencies pointing inwards, the domain free of Smithay, the filesystem and the clock, crates doing what `docs/ARCHITECTURE.md` says, and the promises kept: the IPC protocol, the schemas, the list of protocols |
| components | `review-components` | what is written twice that could be one, tested once (the DRY review), and code quality clippy cannot see: names, module boundaries, comments that say why, the rules of `AGENTS.md` |
| performance | `review-performance` | work when nothing changes, allocation in hot paths, what grows with the number of windows, screens or programs |
| reliability | `review-reliability` | what fails and how it is recovered from: errors swallowed or turned into panics, restarts, a session outliving its compositor, races between threads and processes |
| experience | `review-experience` | sensible with no configuration, a config mistake never breaking the session, what the user is told and in what words, keyboard reach, accessibility and translation |
| tests | `review-tests` | tests that would pass with their fault in, tests of implementation rather than behaviour, what is covered only by being run and never asserted, flakiness waiting to happen |
| docs | `review-docs` | ADRs, the roadmap, the architecture, the guide and the manual pages against the code: what they say that is no longer so |

## 2. Start the reviewers

Start one agent for each dimension, all in one message, so that they run
at once. Each prompt says only:

- the scope, exactly (the range, the crates, or the whole tree);
- the question, if any;
- what is already known and is not to be found again: the open findings
  of earlier reviews in `docs/reviews/` and the roadmap's items that
  cover the same ground.

Each agent reads `.claude/skills/review/reviewer.md` first, which says
how to work and how to answer. They read; none runs the gates or the
whole suite, which share the machine with whatever else is running. A
reviewer may run one test by name to show a finding.

While they run, do nothing that changes the working copy.

## 3. Check what comes back

A reviewer's finding is a claim. Before it is written up:

- open the code it names and see that it says what the finding says;
- where it is marked **shown by a test**, run that test, with the fault
  as it is, and see it fail;
- drop what is not so, and say in the write-up what was dropped and why
  if a reader would otherwise look for it;
- merge what two reviewers found from two sides into one finding;
- rank: **high** stops or ends the session, or gives away what is
  private; **medium** costs without bound, or breaks a promise;
  **low**, the rest.

## 4. Write it up

One file, `docs/reviews/<date>-<what-was-reviewed>.md`, in the shape of
the review of 15 November 2026: what was asked and of whom; the
findings, highest first, each with where it is, how it was shown, what
to change and the test to write first; what was looked at and found
sound, so that it is not looked at again for the same thing; and what
was not looked at.

Then the roadmap: an item for each finding under the stage it belongs
to, linking the finding's heading, in the order to take them. The links
test (`cargo test -p omorfos-config --test links`) checks the headings.

Commit the write-up and the roadmap alone, as `docs: review ...`, with
the `commit` tier.

## 5. Say what was found

To the maintainer: how many findings at each rank, the highest in a
sentence each, what was dropped, and what was not looked at. A finding
that lets an ordinary program stop or end the session, or read what it
should not, is said first, with a recommendation to fix it before the
write-up is pushed, as the write-up shows how.
