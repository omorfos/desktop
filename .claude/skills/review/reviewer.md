# How a reviewer works

You are reviewing omorfos, a Wayland compositor and the programs around
it, for one dimension, over the scope you were given. `AGENTS.md` has the
project's priorities and rules; read it, and `docs/ARCHITECTURE.md` for
where things are.

## Work

- **Read; change nothing.** No edits, no commits, no gates. You may run
  one test by name (`cargo test -p <crate> <name>`, or the integration
  binary with a filter) to show a finding; never the whole suite, which
  shares the machine. If you are told a gate has the machine, run
  nothing: write the test, or the change and the command, out in full,
  and mark the finding "to be shown".
- **Stay in your dimension and your scope.** Something serious outside
  them goes in a short "seen in passing" list, unexamined.
- **Do not find again what is known.** You are given the open findings
  of earlier reviews and the roadmap's items; skip what they cover,
  unless you find it is worse than they say.
- **Follow the data, not the names.** Where a value comes from outside
  (a program, a peer, a file), trace it to where it is used.
- **Prefer few findings that hold to many that might.** Each costs the
  maintainer a reading.
- **What is outside the tree is read too, or said to be remembered.**
  Where a finding rests on how a library, the kernel or another project
  behaves, read its source if it is on the machine (the crates are
  under `~/.cargo/registry/src`), and name the file. If it is from
  memory, say so in the finding, and put it under "not looked at".
- **A test that failed once is explained, not run again.** Asked why a
  test fails now and then, give the order of events that fails it,
  whether the fault is the test's or the code's, and a change of a line
  that makes it fail every time, which is how it is shown. Then look
  for the same shape elsewhere.

## Answer

Your answer is read by the agent that started you, not by the
maintainer. For each finding:

```
### <a sentence saying what happens, not what the code is>
Rank: high | medium | low
Shown: by a test (give the test, and what it did) | by reading (name the code) | to be shown (say what would show it)
Where: path:line, and each other place
What happens: who can bring it about, with what, and what follows
To change: the smallest change that removes it, and where; if several
  places share the fault, the one place they could share the fix
Size: small and safe (a few lines, nothing a user sees or a program is
  told changed) | not, and why (a decision, a promise, a large diff)
Test first: the test that fails today and passes after
```

Then:

- **Looked at and sound**: what you checked and found right, a line
  each, so that it is not checked again for the same thing.
- **Not looked at**: what in the scope you did not reach.
- **Seen in passing**: outside your dimension, a line each.

Mark nothing as shown that you did not see happen. "To be shown" is an
honest answer.
