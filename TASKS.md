# watchglass — tasks

Ordered. Each task names what proves it done. Status: `[ ]` todo, `[~]` in
progress, `[x]` done. Keep this file current.

## v0.1 (superseded 2026-09-27)

Configured commands under app-owned PTYs. Kept: the terminal engine,
Ghostty import, follow/pause, markers, jump, search, captures, the UI
chrome. Dropped: `watchglass.json`, spawning, source detection.

## v0.2 tmux + Claude + Jev

- [x] T20 tmux discovery: `list-panes` poll, panes channel, parse tests. Proof: all five live panes listed in the app.
- [x] T21 Taps: `capture-pane` snapshot, `pipe-pane` to a tap file, tail thread, detach, close on quit. Proof: `#{pane_pipe}` is 1 on every pane while the app runs; tap files grow.
- [x] T22 Claude sessions: state files, transcript tail, dead-pid filter, 2 s poll. Proof: unit tests over a fixture transcript; sidebar lists live sessions.
- [x] T23 Jev classifier: client, five-level Choice for panes and sessions, rules for `waiting`/`busy`, per-id debounce, unchanged-tail skip, `verdicts.log`. Proof: unit tests; log lines appear while the app runs.
- [x] T24 Mirror tiles: fixed cols×rows from tmux, font fit 5–22 px, resize follows the pane, header shows level, command or Claude name, go-to button. Proof: mock screenshots.
- [x] T25 Claude sidebar with priority sort, snippet, go-to; top bar counts, order toggle, Jev status. Proof: mock screenshots.
- [x] T26 Notifications on rising verdicts only, rate limited.
- [~] T27 Verify in the real window. User's screenshot 2026-09-27 16:00 showed the sidebar, Approve/Reject on the waiting session, the idle strip, and one tile rendering; the other tile was blank. Cause: the screen snapshot went over the chunk channel and was skipped when a tile remounted on an already-attached pane. Fix: `attach_pane` now returns the snapshot and the tile queues live chunks until it is written. Re-check pending.
- [x] T28a Idle shells collapse into a strip below the grid; click a chip to show one; a shell that starts a command expands on its own. Proof: mock screenshot with two collapsed shells. Per-tile zoom still open.
- [x] T29 Approve / Reject on a `waiting` session sends `1` or `Escape` to its pane, the same keys claude-sessions.nvim uses. Buttons only appear while waiting. Proof: mock renders them; real prompt not yet exercised.
- [x] T32 Tap hygiene 2026-09-27: tap files rotate at 4 MB once read; on startup, pipes that point at our own tap files are closed and litter for dead panes removed. Proof: Rust test `cleanup_closes_only_our_stale_taps`; dev log shows "closed stale pipes" after a restart.
- [x] T34 Typing without becoming a terminal 2026-09-27: reply box on every Claude session with a pane (`send-keys -l` then Enter), per-tile `›` line box (`i`), Enter and Ctrl-C buttons, and ⇧N "new pane" opening a tmux window that appears as a tile. Proof: 2 Rust tests for the argument shapes; the demo recording exercises reply, and new pane against the mock.
- [x] T35 Shell rule: a pane whose foreground command is a shell is `idle` by rule, never sent to Jev. Found when an idle zsh pane showing Claude's exit screen was judged "needs you". Proof: Rust test; the pane collapsed after the restart.
- [x] T36 Demo video: `pnpm demo` drives the mock in Chrome with captions and records it; `pnpm demo:encode` makes `docs/demo.mp4` and `docs/demo.gif`. Proof: frames inspected at six timestamps.
- [ ] T33 Classifier debounce cap of 10 s is in; a global Jev budget for many panes is not.
- [ ] T30 Shell hook for processes outside tmux.
- [ ] T31 Replay a capture from the UI; packaging with `pnpm tauri build`.
