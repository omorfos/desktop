# The pull requests merged on 23 November, reviewed

- Date: 2026-11-24
- Scope: the 32 pull requests merged into `main` on 23 November 2026,
  #67 to #78 and #83 to #102, as their 48 commits are on `main` at
  `41cda51`. Twenty-four were merged in the first two hours of the day,
  most of them the fixes of the reviews of 22 November; eight, #95 to
  #102, in its last hour, the fixes of
  [the review of 23 November](2026-11-23-the-whole-tree-for-safety-speed-and-use.md).
  The 21 commits pushed to `main` that day without a pull request are
  not in it.
- Asked of: all eight reviewers (security, architecture, components,
  performance, reliability, experience, tests, docs), each over the
  whole scope, asked whether each pull request does what it says, is
  right, and broke or left out anything. None ran anything, as the gates
  had the machine.
- How it was shown: each finding was opened in the code by the agent
  that started them. Those marked **shown** were then run: a test
  written for the finding, with the fault as it is and, where the fault
  came with a pull request, on the commit before it; a fault put back to
  see whether any test notices; a count of allocations; a measure of
  time. The rest are **by reading**, and say what they rest on.

## What was run on `main`

- **Everything, on `41cda51`**, by `tools/gates.sh push`: formatting,
  clippy, every test under the coverage gate, at 100% of 158,817 lines,
  10,747 functions and 79,282 regions, the doc tests, `cargo deny`, the
  workflows and the documentation. Not the tests that need a graphics
  card, which WSL has none of.
- **Each commit alone**, by `tools/gates.sh commits`, from `83e147a` to
  `a56800e`: the 20 commits of #95 to #102 and `4def181` among them,
  each built and what it reaches tested (ADR 96). All pass.
- **What was merged is what was gated.** Each of #95 to #102 as its
  branch passed the gates, against its commits on `main`, by
  `git range-diff`: the same, commit for commit.
- **CI.** Every `Test` and `Lint` run on `main` that finished on
  23 November passed; the others were cancelled by the push after them.
  `41cda51` passed all four workflows.
- Not run: each commit of #67 to #94 alone. CI passed on `main` after
  them.

`main` passes everything with findings 1 to 5 in it. None of them fails
a test, which findings 4 and 8 are about.

## Findings, highest first

### 1. One region set on many surfaces is counted once by the relay and copied into each surface by the compositor, so one program can still fill the compositor's memory

High. Of #68, which bounded what a connection adds. Shown, on both
sides.

- **Where:** `crates/omorfos-relay/src/journal.rs:589-599` counts what
  is added to an object, `Rule::Adding`, and nothing else;
  `wl_surface.set_opaque_region` and `set_input_region` are
  `Rule::Pending` (`:106-112`), and the regions of blur and of the
  pointer `Rule::Lasting`. Smithay 0.7.0 copies the region's whole list
  into the surface at each (`src/wayland/compositor/handlers.rs:241-264`),
  and again as the surface is committed (`:111-121`):
  `RegionAttributes` is a `Vec`, cloned.
- **What happens:** any program, one in a sandbox included, needs only
  `wl_compositor`. It makes one region of 4096 rectangles, which is all
  the relay counts of it, then surfaces, and sets the region as each
  one's opaque and input region and commits: 44 bytes a surface. The
  compositor then holds the list four times for each, about 324 KiB. A
  connection may have 2^18 objects (`objects.rs:20`), which is some
  87 GB; 16 GiB takes about 52,000 surfaces, 2.3 MB of requests. What
  follows is as finding 1 of 22 November had it, which #68 put right for
  the relay's own memory and for regions counted by their rectangles:
  the kernel ends the largest process, and the relay hands the next
  compositor the same surfaces. `docs/UPSTREAM.md:31-36` says a program
  connected through the relay "never reaches Smithay with more".
- **Shown:** in `crates/omorfos-relay/src/link.rs`, a program fills the
  region 4 and has 1000 surfaces name it twice each and commit: the
  relay refuses none. In the compositor's tests, a client does the same
  with 10 surfaces and with 100, and what the compositor holds after,
  by `allocation_counter`, is 3,318,800 bytes and 33,189,776: 331,880
  for each.
- **To change:** count each naming, where every naming passes
  (`Journal::name` and `unname`, `journal.rs:1096-1116`): what is
  counted against `MOST_ADDED_IN_ALL` is then what was added to each
  object times one more than how often something kept names it, which
  is what the compositor is made to hold. No program that is not trying
  comes near it. `UPSTREAM.md` to say that Smithay copies a region into
  each surface it is set on, which an `Arc` there would end. Not small:
  what the count means changes, and it must not be able to go below
  nothing.
- **Test first:** beside
  `a_program_may_not_add_more_in_all_than_any_adds_by_spreading_it_over_objects`:
  one region as full as one may be, set as the opaque region of one
  more surface than `MOST_ADDED_IN_ALL / MOST_ADDED`; the last is
  refused with `ADDED_TOO_MUCH`. Today every one is read.

### 2. A window over one in a frame of ours no longer eases where it is sent, and every turn makes three maps anew that it then throws away

Medium. Of #102. Shown. Put right in #103, by taking #102 back.

- **Where:** `crates/omorfos/src/compositor.rs:2707` notes where each
  window is in the last scene, once, before the loop of `sync`;
  `:2723-2725` looks there; `:2733` calls `painted` (`:6130-6135`)
  inside the loop, which takes an entry out of that list with
  `swap_remove`, moving the last into the room left. The maps are made
  at `:2707` and, in `portray`, at `:2808`.
- **What happens:** the entry moved is looked for where it was, past
  the end, and not found, so its window is taken for new on every turn.
  `travel` (`:6543`) then starts it at rest where it was sent, and
  `stretching` has nothing to carry on. With two framed windows as they
  are opened, it is the newer, the one with the keyboard: maximized,
  restored or tiled, it is there at once, and its picture pops to the
  new size. The built-in theme has windows change over 180 ms, so this
  is every session with two windows. Most of the reviewers found
  it, each alone. And `sync` runs in every `refresh` (`:1388`), so each turn,
  with nothing changing, now makes two maps there and a third in
  `portray`, of every window, though nothing is being captured.
  `SyncRoom` is there "so that a turn in which nothing came or went
  allocates nothing for it".
- **Shown:** a window opened over a framed one, with windows changing
  over a time, and maximized: on `923fa6b`, before #102, it sets out
  from where it was; on #102 it fills the screen at once, framed itself
  or not. A turn with nothing changing and one framed window allocated
  13 times before #102 and 16 with it; with a glass titlebar 20 and 23;
  three more at 2, 8 and 32 windows alike.
- **What it bought:** nothing that can be measured. #102's numbers were
  of lookups in lists of numbers, written for the purpose and not kept.
  Measured on the compositor, in the `nightly` profile under WSL, the
  windows open and nothing changing, 200 turns at a time, the least of
  41 such; the least and the most of three runs of each:

  | Framed windows | Before #102 | With #102 |
  |---|---|---|
  | 1 | 5.05 to 5.11 µs | 5.06 to 5.11 µs |
  | 8 | 19.4 to 19.5 µs | 19.4 to 19.5 µs |
  | 32 | 69.3 to 69.6 µs | 68.9 to 69.7 µs |
  | 128 | 351 to 353 µs | 347 to 355 µs |
  | 256 | 900 to 909 µs | 903 to 922 µs |

  A turn does grow faster than the windows, 2.2 µs a window at 32 and
  3.5 µs at 256, but not for the two walks #102 replaced: `painted`
  still walks the last scene for each framed window, and Smithay's
  `Space::map_element` searches and sorts for each (finding 30 of
  16 November). At 256 windows a turn is a twentieth of a frame at
  60 Hz.
- **To change:** take #102 back, which #103 does, with the test and a
  comment, where the entry is looked for, on why it is by its id; and
  lower `quiet.rs`'s ceiling to the 13 a turn allocates, which #103
  does too: it stood at 17, with room for the three. Finding 3 of
  23 November then stays open with what was measured beside it. The
  measure, for `tests/integration/quiet.rs`, run with
  `cargo test --profile nightly -p omorfos --all-features --test integration how_long_a_turn -- --ignored --nocapture`:

  ```rust
  #[test]
  #[ignore = "a measurement, run by hand in an optimised build"]
  fn how_long_a_turn_with_nothing_changing_takes_as_windows_are_added() {
      for windows in [1_usize, 8, 32, 128, 256] {
          let mut session = Session::with_config(&[("theme.css", SOLID)]);
          let mut client = session.connect();
          let _windows: Vec<_> = (0..windows)
              .map(|each| {
                  let app = format!("app{each}");
                  session.open_decorated(&mut client, &app, (200, 100), 0xff00_ff00)
              })
              .collect();
          session.eventually(|session| !session.render());
          let mut samples = Vec::new();
          for _ in 0..41 {
              let started = std::time::Instant::now();
              for _ in 0..200 {
                  session.advance(Duration::from_millis(16));
              }
              samples.push(started.elapsed() / 200);
          }
          samples.sort();
          let said = format!("{windows}: least {:?}, middle {:?}\n", samples[0], samples[20]);
          std::io::Write::write_all(&mut std::io::stderr(), said.as_bytes()).unwrap();
      }
  }
  ```
- **Test first:**
  `a_window_over_one_in_a_frame_of_ours_eases_where_it_is_sent_as_one_alone_does`,
  in #103.

### 3. A locked frame that was drawn and then refused by the card still counts its screen as showing nothing of the session

Medium. Of #85. By reading: it is in the hardware backend, which no
test runs, and rests on Smithay's source.

- **Where:** `crates/omorfos/src/compositor.rs:3305` and `:3745` set
  `blanked` by whether `render_output` failed.
  `crates/omorfos/src/backend/hardware.rs:1298-1313` draws first and
  then hands the buffer over, `queue_buffer(..)?`; `:782-797` goes on
  after a failure. `confirm_lock` (`lock.rs:342`) reads `blanked`.
- **What happens:** #85's message names the frame lost when "another
  terminal has the screen". While Smithay knows the card is not ours,
  that frame fails in `next_buffer`, before anything is drawn
  (`smithay-0.7.0/src/backend/drm/surface/gbm.rs:254-256`), and
  `blanked` is not set, before the fix or after. In the moment before
  it is told, which `hardware.rs:779-781` says is the commonest cause,
  the frame is drawn, `blanked` is set, and the kernel refuses the flip
  inside `queue_buffer` (`gbm.rs:296-313`, `:393-400`). The next turn
  tells the locker the session is locked, and `omorfos-session` lets
  the machine sleep, with the session's last frame the last the screen
  was given. The roadmap's open item on a frame "shown, not queued"
  takes the gap left to be the frame queued and not yet shown; a frame
  never queued counts too.
- **To change:** a covered `Compositor::not_shown(&Output)` that sets
  `blanked` false for that screen, or the mirror that is it, called
  from the arm at `hardware.rs:787`; one line in the excluded file,
  with no decision in it. It goes with finding 1 of the review of what
  the coverage gate leaves out: until a refused screen is tried again,
  the lock is then said to be locked only by whatever next wakes the
  loop. The roadmap's item, done in full, would take its place.
- **Test first:** in `tests/integration/lock.rs`: locked, a frame
  drawn, `not_shown` for the screen, and the locker is not told it is
  locked until the next frame is shown.

### 4. The launcher, the screenshot tool and the welcome card still start from the files on disk, and nothing holds the lock screen, as it is started, to the copy

Medium. Of #95, and of what #101 says of it. By reading for the three;
shown for the lock screen.

- **Where:** `crates/omorfos-launcher/src/client.rs:90` and
  `crates/omorfos-screenshot/src/lib.rs:197` open the configuration
  with no `starting_from`; `crates/omorfos-welcome/src/lib.rs:99` loads
  it over the defaults. `crates/omorfos-lock/src/lib.rs:146` and `:158`
  are where the lock screen takes the copy.
- **What happens:** #95 put the lock screen right, the one program the
  review of 23 November named, and that review lists as sound "started
  from by every part but the lock screen" (`:272-274`), which was
  written without each program being looked at. With `config.yaml` or
  `theme.css` saved with a mistake, the three started when asked for
  fall to the defaults, where the guide says the file's "previous
  contents" go on being used: the launcher comes up in the built-in
  theme and settings; a picture of the screen is saved to a file where
  `screenshots.destination: clipboard` was set, and nothing is copied;
  the welcome card lists the built-in keys. Each of a dozen places says
  for itself whether it starts from the copy, by spelling
  `starting_from` out or not, which is how these were missed. And of
  the lock screen: its tests call `configuration` and
  `Lock::new` with a copy themselves, and the one test that runs
  `start`, which is what the program runs, has a file that loads.
- **Shown:** with `start` giving `None` for the copy where it reads the
  ways and the power offer, and again where it makes the lock screen,
  every test of `omorfos-lock` and of `lock_screen::` passes.
- **To change:** one way in, in `omorfos-config`, that takes the
  configuration's directory and `XDG_RUNTIME_DIR` and cannot be used
  without the copy, for every program but the compositor, which keeps
  the copies. The three take it. ADR 3 (`:49-55`) to name them and the
  lock screen. Whether the welcome card, shown at a first login, is to
  read the copy is a decision.
- **Test first:** `theme.css` broken, a kept copy whose `launcher` has
  a background of its own, and the launcher drawn in that colour. And
  for the lock screen, the test that runs `start`, with `config.yaml`
  broken, `fingerprint: false` in the copy and a finger on the reader
  all along: the session stays locked.

### 5. An ordinary notification that comes while eight critical ones show is itself the one to go, and is never seen

Low. Of #100. Shown.

- **Where:** `crates/omorfos-notifications/src/note.rs:239` puts what
  has come first, and `:247-253` then takes away the oldest that is
  not critical.
- **What happens:** with eight critical notifications showing, which
  stay until clicked, the only one not critical is the one that has
  just come. It goes in the call that shows it, and its program is told
  it expired (`client.rs:191-194`). Until one of the eight is clicked
  no other is shown, the level `omorfos-session` shows for the volume
  and brightness keys among them; and any program, or `notify-send -u
  critical`, can bring it about. Before #100 what came was always
  shown. Most of the reviewers found it.
- **Shown:** eight critical, then one that is not: `show` gives back
  its own number. On the commit before #100 it is shown.
- **To change:** a decision between two promises, that a critical one
  stays and that what is sent is shown. Recommended: what has just come
  is never what makes room for itself. The search runs over the
  others, so with nothing else ordinary the oldest critical goes, which
  is what #100 already does when the newcomer is critical too; the
  guide's sentence then reads as it stands.
- **Test first:** eight critical, then `note(9, ..)`: `show` gives
  `Some(1)`, and `[9, 8, 7, 6, 5, 4, 3, 2]` are showing.

### 6. A configuration directory's changes are still missed, or mistaken, four ways

Low. Of #69. Shown for the first two; by reading for the others.

- **Where:** `crates/omorfos-config/src/watcher.rs:189-200` and
  `:210-218`.
- **What happens:**
  - The directory is a link, as into a checkout of dotfiles, and what
    it points at is taken away and made again. The watch goes with the
    directory; `watch_again` fails, as the path leads nowhere; and
    when it leads somewhere again nothing says so, as the link's own
    name never changed. No edit is seen until the next login.
  - The directory is moved aside, as `mv omorfos omorfos.bak` to try
    the defaults. That it went is not taken for a change, and the old
    watch stays on what it became: a file saved there later has every
    program load from the empty path, the defaults.
  - A queue that overflowed is taken as files changed, but not as the
    directory replaced, though its going may be what was lost.
  - The directory above is itself replaced, as `~/.config` moved aside
    and put back: both watches stay on what was there before.

  The type's comment and ADR 3 say it "is watched again wherever its
  path then leads", which reads as covering the first.
- **Shown:** a link whose directory is removed and made again, then a
  file written in it: no change is said, where with a directory that is
  no link one is. A directory renamed away: no change is said; a file
  written in what it became: one is.
- **To change:** in `drain`, the directory come, gone or moved is a
  change whether or not it can be watched again; where it cannot,
  `watch_again` lets go of the old watch and is tried again at each
  `drain` until it holds; an overflow watches again too. The last case
  needs watching further up, or saying in ADR 3 that it is not seen.
- **Test first:** the two shown, in `watcher.rs`.

### 7. What every peer asks in a turn is now read into memory before any of it is answered

Low. Of #87. By reading.

- **Where:** `crates/omorfos/src/ipc.rs:343-349`.
- **What happens:** before, each line was decoded, answered and let
  go. Now every decoded request of every peer is kept until all are
  read: 32 peers by 64 lines a turn. `Perform` carries a
  `serde_json::Value`, and a line as long as one may be, 64 KiB of
  `[0,0,...]`, decodes to about a megabyte. The most a turn holds goes
  from a few megabytes to about two gigabytes, let go at its end. It
  takes a program of the user's outside a sandbox, which the roadmap's
  item on who may use the socket already weighs; but `ipc.rs:4-7` says
  peers cannot do it.
- **To change:** only taking peers on needs the server out. With it
  back, each peer's lines are read, decoded and answered one at a time,
  as before, in the order asked.
- **Test first:** 32 peers each send a `Perform` nearly as long as a
  message may be; the most held in the turn, by `allocation_counter`'s
  `bytes_max`, is under 8 MiB.

### 8. Six of the day's fixes are run by the tests and held by none

Low. Of #69, #72, #75, #84, #85 and #90. Shown: each fault put back,
alone, and every test that reaches it passes.

- **#84**, each turn told the time once it has waited: with the time
  read before the wait again (`crates/omorfos/src/session.rs:259`), all
  113 of the compositor's own tests and all 1199 of the integration
  binary pass. Its fault was two turns for every timer and frame of an
  animation, and a first key after a long idle that had to be pressed
  twice.
- **#85**, a screen counted as showing nothing only once its locked
  frame drew: counted whether or not it drew (`compositor.rs:3305`,
  `:3745`), the 47 tests of `lock::`, `lock_screen::` and `mirroring::`
  pass. Its commit says the tests' renderer does not fail; a frame
  fails without it, where the damage tracker is of an output with no
  mode.
- **#72**, a gone screen's global let go of when it is due: with what
  was withdrawn forgotten first in the turn again
  (`compositor.rs:1404`), the 43 tests of `outputs::` and
  `screens_off::` pass. The new test takes the screen away before a
  turn, where first or last is the same; a screen turned off by a
  reload inside the turn is what the move is for.
- **#75**, the locale looked up in one place: with the panel asking by
  `LC_MESSAGES` for the week's first day, and the launcher for the
  decimal sign (`omorfos-panel/src/panel.rs:160`,
  `omorfos-launcher/src/lib.rs:75`), 204 tests pass. The cases that
  held the category went with the copies they were tests of.
- **#90**, the bar's time and day read from one moment: with the day
  always 1 January 2000 (`omorfos-shell/src/clock.rs:15`), 95 tests
  pass.
- **#69:** with the directory's own watch going or moving not looked
  at (`omorfos-config/src/watcher.rs:193-194`, `:229`), the 17 tests of
  the watcher pass: in both of #69's it is the name in the directory
  above that says so.

Nothing to change in the code. The test to write for each:

- #84: `dispatch_now` with a wait a little past `deadline()`: what was
  due is done in that turn, not the next.
- #85: a frame drawn with a tracker of an output with no mode, which
  fails; locked, the locker is not told so after it, and is after the
  next that is shown.
- #72: two screens, one turned off in `config.yaml`; five seconds on,
  nothing is due.
- #75: one function in each crate that takes the environment:
  `LANG=pl_PL` with `LC_TIME=en_US` gives Sunday; `LANG=en_GB` with
  `LC_NUMERIC=pl_PL` gives a comma; `LC_MESSAGES` alone gives neither.
- #90: 23:59:59.999 reads "23:59" and that day; a millisecond on,
  "00:00" and the next.
- #69: the first test of finding 6, which needs the line.

### 9. A program let go for what it added over all its objects is told it added too much to one

Low. Of #68. By reading.

- **Where:** `crates/omorfos-relay/src/journal.rs:589-591`;
  `wire.rs:66-68`.
- **What happens:** the new bound refuses with the old reason, "more
  added to one object than any program adds", which the program is
  told and `session.log` keeps. Whoever reads it looks for one object
  with too much and finds none.
- **To change:** a reason of its own in `MORE_THAN_KEPT`, returned
  where `in_all` is what refused; the error's number stays.
- **Test first:** the words, in
  `a_program_may_not_add_more_in_all_than_any_adds_by_spreading_it_over_objects`.

### 10. The check #97 gave `omorfos_shell::pixels` is that function's alone

Low. Of #97. By reading.

- **Where:** `crates/xdg-desktop-portal-omorfos/src/feed.rs:179-183`,
  `:568-571`, `:878-881`; `crates/omorfos-screenshot/src/client.rs:444-460`.
- **What happens:** the portal makes a pool and a buffer by hand in
  three places, with the `unwrap_or(i32::MAX)` and the unchecked
  `across * 4` that #97 took out of `pixels`, and the screenshot tool
  has a way of its own. The sizes are what the compositor says a
  picture is, so, as the finding said of `pixels`, it is a promise not
  kept, not a fault seen.
- **To change:** `checked_layout` public, returning a named layout
  rather than four numbers in a row, and taken by all five.
- **Test first:** its four tests move with it; memory for pictures of a
  screen 2^30 wide is refused by the portal, not made.

### 11. The review merged in #101 says six things that are not so

Low. Of #101. By reading.

- **It carries as an open High what ADR 58 decided.** Finding 9 is
  that a program can start a `gpg-agent` of its own and ask through
  it. ADR 58 says so, and that the check on the pinentry's parent
  "raises the bar, and is not the boundary". The first write-up said
  that; the second made it a finding and took the sentence out, and
  tells the reader to "amend ADR 58 with the chosen same-user threat
  boundary". What is open is whether to go back on that decision,
  which is the maintainer's to rank.
- **Finding 10** says the click is sent "for every left click". It is
  sent for one on a notification whose program offered `default`
  (`crates/omorfos-notifications/src/client.rs:680`).
- **"Every part but the lock screen"** starts from the kept copy:
  finding 4 above.
- **Finding 4** asks for capture's scratch space to be kept. It is
  (`crates/omorfos/src/capture.rs:399-401`); what is left of the
  finding holds.
- **Finding 6** says every caller of `pixels` passes a canvas's own
  size and bytes. The wallpaper passes its cache's
  (`omorfos-wallpaper/src/wallpaper.rs:824`), which are checked
  (`cache.rs:75`).
- **The tree it names**, `7f27ec2`, is on no branch `main` has, and is
  not what was read: it has no ADR 103 and no `MOST_ADDED_IN_ALL`,
  which the review speaks of. What was read was a later `main`, at or
  near `861a4fd`.
- And of its words: "The findings remain open: the review skill's
  **high** rank stops" runs two sentences together; "It was reverted"
  is said of a check that was never on `main`; on the roadmap the High
  comes last, after four Lows, under "in the order to take it"; and
  "Next" still names what the reviews of 22 and 19 November found
  unsafe, each of which is ticked.

### 12. Documents and comments the day's changes left behind

Low. By reading.

- **#67 to #78:** the review of 22 November still says "Written, not
  built" of ten findings that are merged and ticked, and the roadmap's
  line over them "to be built and tested first"; the fixes of
  15 November each marked their finding. Their commits end "to be
  built and run before this is taken", which cannot be changed now.
- **#94:** its table of decisions, "about a hundred", counts `if` and
  `match` on every line, the comments' and the shader's own `#if`
  among them: 103 where the lines of Rust have about 70, and none in
  `shader.rs`. Finding 11's budget, counted with `syn`, will not match
  it.
- **#96:** `docs/DESIGN.md:362` still asks what a new part does "under
  each of the three settings above", with the third gone; and the row
  taken out was the only place that said what high contrast is to do.
  Its words belong in `ACCESSIBILITY.md`, under what is not there yet.
- **#95:** ADR 3 names who starts from the kept copies, without the
  lock screen.
- **#87:** `crates/omorfos/src/ipc.rs:315-316` says the server is out
  "while requests are answered", which is what #87 ended.
- **#100:** `note.rs:13-14` says "the oldest goes".
- **#69:** `Watcher::drain`'s comment says when it returns `true`,
  without the directory being watched again.
- **#73:** a start of the X server that did not fail is given the name
  `"x11-cannot-run"`, which nothing reads (`x11.rs:573-578`).
- **#74:** `omorfos-clipboard/src/lib.rs:290-293` does not say its 32
  is under the shared most, as finding 10 asked; `tests/integration/x11.rs:1313`
  has `255` written out.

### 13. Six commit messages do not hold to AGENTS.md, or do not stand alone on `main`

Low. By reading. Nothing to change on `main`, as it is pushed; for the
routine.

- #100, `923fa6b`: a subject of 74 characters, typed `docs`, which
  also renames two tests and takes out an assertion. `committed`
  passed it: 45 of `main`'s 876 subjects are longer than 72.
- #102, `a3449dd`: scope `compositor`, used this once; the crate's is
  `omorfos`.
- #95, #97 and #98: one subject and body three times, "leave this
  finding to the review's own section", which names nothing once it is
  on `main`.
- #98: a change and its revert both reached `main`, and the revert
  names the commit as its branch had it (`5d726a9`), which `main` has
  as `91bd5ad`. A pull request's fix-ups are better squashed before it
  is rebased onto `main`.

## Dropped

- **That `main` fails its own gate for allocations since #102.** Three
  reviewers took `quiet.rs`'s ceiling of 17 to be what a turn
  allocates, as its comment says, and 20 to fail it. A turn allocated
  13, so 16 passes: finding 2 has it as it is, and #103 lowers the
  ceiling to 13.
- **That an X11 test fails under WSL for want of `DISPLAY`**, as #102's
  description says. Under WSL the directory X servers keep their
  sockets in cannot be written to; the gates give the tests one of
  their own, and the test passes in them.

## Looked at and sound

- **#68**, but for findings 1 and 9: the count goes up only where a
  request is kept and down in the one `remove` every letting-go passes,
  so it cannot drift.
- **#70, #71, #73, #74, #89, #90, #91:** do what they say; nothing a
  user is shown changes but what each names.
- **#76, #77, #78, #86, #87, #91, #92, #97, #98, #99 and #100's own
  two:** each test fails with its fault in. Shown for #77 and #78,
  whose commits left it "to be shown"; by reading for the rest.
- **#83 and #94**, but for the count: what they say of the code holds
  at the lines they name.
- **#93:** the guide for WSL agrees with what the nested backend
  prints.
- **#95 to #101** as documents, but for finding 11: every tick is true
  of `main`, and every link resolves.

## Not looked at

- Anything on a graphics card or a real screen: #84, #85 and #93 were
  read only, and finding 3 rests on Smithay's source.
- Each commit of #67 to #94 alone.
- Whether allocation counts are the same on `aarch64`.
- The 21 commits pushed without a pull request, but for where they
  moved lines.

## Seen in passing, for later

- `portal::it_ends_when_the_bus_does_and_needs_no_configuration_it_can_watch`,
  which fails now and then in full runs, is still not explained. The
  reliability reviewer's reading, that the test pulls the bus before
  the portal has finished starting and then waits too few turns, was
  tried: with six seconds put before `Desk::new`, the test passes three
  times in three, in 6.2 seconds. The roadmap's item on it stays open.

- `tools/gates.sh commit` and `commits` fail 31 `ask::` tests in a
  build folder with nothing in it, for a change that reaches `omorfos`
  and not `omorfos-ask`: the tests look for `omorfos-pinentry` beside
  their own binary, and `cargo test -p omorfos` does not build it.
- `Client::receive` in the tests now returns what reading says.
  wayland-backend's own client says `WouldBlock` for nothing to read
  (`src/rs/client_impl/mod.rs:650-654`); the tests have libwayland's,
  through winit, where that is no error.
- A program that never acknowledges a configure: the relay's
  `Window::offered` and Smithay's `pending_configures` grow by each one
  it is sent, and it can ask for them. Unexamined.
- #85: where a screen's locked frame keeps failing to draw, the locker
  is never told the session is locked, and `omorfos-lock` draws and
  asks nothing until it is. Unexamined.
- #91: leaving the empty workspace past the last by a key has nothing
  beside for the slide. To be shown.
- #70: the question whether to leave, and the screens' question, take
  Enter and Escape whatever is held, as "About" did.
- ADR 58 and `docs/guide/passwords.md` still say GnuPG's words are in
  the session's log, which `3b41e24` took out.
- `last-good` is emptied at logout only where `XDG_RUNTIME_DIR` goes
  with the login: with lingering it stays, and the next login starts a
  broken file from the last login's copy.

## The fixes, read again

The pull requests that answer these findings, #103 to #115, were read
again the same day, each against its finding, by the agent that started
them. Nothing here was merged then. What was run:

- **All thirteen together**, merged onto `main` at `1fba41b`: they merge
  with nothing to put right by hand, and every test passes on the whole,
  3,020 of them, with #103's ceiling of 13 allocations a turn holding
  over the other twelve. Not the tests that need a graphics card, nor
  the coverage gate over all of them at once, which each passed alone.
- **Each fault put back**, for one run, where a new test is there to
  hold it: the lines of #69, #72, #84 and #85, `not_shown` doing nothing,
  #87's reading of every peer before it answers one, and the lock
  screen, the launcher, the screenshot tool and the welcome card without
  the copy. Thirteen tests, each of which then fails.
- **#112 in a build folder with no `omorfos-pinentry` in it**: its gates
  build it and the 33 tests of `ask::` pass, where `main`'s script fails
  31 of them.

Three things were found, and each was put right in the pull request it
was found in:

- **Finding 1's bound could be got past by taking a number again
  early.** High. Of #104. Shown. The relay withholds the compositor's
  word that the number of something destroyed is free while something
  kept still names it, but its table of the program's objects took that
  word at once (`crates/omorfos-relay/src/objects.rs:143-150`), so a
  program that did not wait to be told could make something else with
  the number, and the journal put the new in the place of the old. What
  was added to the old was then counted no longer, though every surface
  it had been set on kept its copy; and what named the old named the
  new, so that setting the new on a surface in place of the old counted
  it once more and once less. An empty region set on 1000 surfaces and
  destroyed, its number taken again and the region filled, and set on
  each surface again: the relay refused none. A full region on as many
  surfaces as there is room for, destroyed, and its number taken again,
  twenty times over: none refused. A request that makes an object with a
  number still kept is now refused, as one made with a number that is
  not free; no Wayland library takes a number before it is told it may.
  The compositor's side is by reading, and is what finding 1 measured:
  a region is copied into a surface as it is set.
- **Finding 6's trying again never came in what sleeps.** Low. Of #108.
  Shown. A directory whose path leads nowhere was tried again at each
  drain of its watcher, which the compositor does every turn of its
  loop. The bar, the wallpaper, the notifications and the portal drain
  it when it can be read, and nothing made it so: with a link's
  directory made again, and then a file saved in it, the descriptor was
  not readable half a second later either time. As the directory's going
  is a change, they had gone over to the defaults by then, and kept them
  until something else woke them. The watcher now becomes readable of
  itself once a second while its path leads nowhere, where nothing else
  would say that it leads somewhere again: for a link, or where the
  directory above may not be watched. A directory that is simply not
  there wakes nothing.
- **#113's notice was not worth what it cost.** Low. By reading. Under
  WSL the compositor said how to have Super where Windows keeps its
  key, at every start and after every reload, until one of fourteen
  options of the keyboard made another key Super. That is what the
  guide says, a section below where it says how omorfos is started
  there. It went on being said to whoever has Super some other way, or
  wants none; it needed the list of options, and a message in every
  language, kept; and the guide beside it called an option the notice
  counted, `altwin:swap_lalt_lwin`, no help. The notice was taken out,
  and the guide says what an option that puts Super on Alt is worth:
  some, but for the chords of Alt that Windows keeps, `Alt+Tab` among
  them.

And two smaller, each also put right where it was:

- **#106**: the way in that cannot be used without the copy is taken by
  three programs of the twelve places that open the configuration; the
  rest spell `starting_from` out, rightly, as before. What was to stop
  the next one being missed is now a test that reads every program's
  source (`crates/omorfos-config/tests/kept_copies.rs`).
- **#111**: the stream's own shared memory was still made as large as
  `width * height * 4` says without a check
  (`crates/xdg-desktop-portal-omorfos/src/stream.rs:318`), which the
  new check then refused to name. It is laid out by the same check now,
  and none is made for a picture Wayland has no numbers for.

The rest do what they say: #103 is `main` as it was before #102, but for
a comment; #105, #107, #109, #110, #112 and #114 had nothing found in
them. Still not seen: #105's one line in the hardware backend on a card,
and the title #113 gives the window on a screen.
