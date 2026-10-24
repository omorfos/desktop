<!--
Thank you. A security vulnerability is not reported here: see SECURITY.md.
CONTRIBUTING.md says what a change is held to; what follows is what a
reviewer needs to know of this one. Leave out what does not apply.
-->

## What and why

<!-- What it changes, and why that is needed: the problem, the issue it
closes ("Closes #..."), or the item of the roadmap it is. -->

## How it was tested

<!-- The tests added or changed, and what they show. For anything that
starts programs or watches files, how many times the tests were run.
What was tried by hand, on which hardware, and what could not be. -->

## What a user will notice

<!-- A setting, a key, a default or a look that changed; anything a user
might not expect. "Nothing" is an answer. -->

## Checklist

- [ ] The gates pass: `cargo fmt`, `clippy` with and without all
      features, `cargo coverage` at 100%, the doctests and `cargo deny`.
- [ ] Each commit follows Conventional Commits, builds and passes alone,
      and says why in its body.
- [ ] What a user can set or see is in `docs/CONFIGURATION.md`, and the
      JSON Schemas are regenerated if the configuration changed.
- [ ] The roadmap is ticked, and a decision has an ADR.
- [ ] A new dependency is justified in its commit and passes `cargo deny`.
- [ ] Security: what this lets a program, an IPC peer or a configuration
      file do that it could not before, if anything, is said above.
- [ ] Accessibility: what it draws can be used with the keyboard alone,
      meets the contrast of `docs/DESIGN.md`, and does not move when the
      theme has nothing move.
