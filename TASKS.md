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
- [x] T28a Hiding is manual (user decision 2026-09-27 evening): every pane is a tile; `h` or the header `–` moves one to the strip, a click brings it back, the set is remembered by session:window.pane. The earlier automatic idle-shell collapse was removed because a freshly opened session vanished into the strip. Per-tile zoom still open.
- [x] T29 Approve / Reject on a `waiting` session sends `1` or `Escape` to its pane, the same keys claude-sessions.nvim uses. Buttons only appear while waiting. Proof: mock renders them; real prompt not yet exercised.
- [x] T32 Tap hygiene 2026-09-27: tap files rotate at 4 MB once read; on startup, pipes that point at our own tap files are closed and litter for dead panes removed. Proof: Rust test `cleanup_closes_only_our_stale_taps`; dev log shows "closed stale pipes" after a restart.
- [x] T34 Typing without becoming a terminal 2026-09-27: reply box on every Claude session with a pane (`send-keys -l` then Enter), per-tile `›` line box (`i`), Enter and Ctrl-C buttons, and ⇧N "new pane" opening a tmux window that appears as a tile. Proof: 2 Rust tests for the argument shapes; the demo recording exercises reply, and new pane against the mock.
- [x] T35 Shell rule: a pane whose foreground command is a shell is `idle` by rule, never sent to Jev. Found when an idle zsh pane showing Claude's exit screen was judged "needs you". Proof: Rust test; the pane collapsed after the restart.
- [x] T36 Demo video: `pnpm demo` drives the mock in Chrome with captions and records it; `pnpm demo:encode` makes `docs/demo.mp4` and `docs/demo.gif`. Proof: frames inspected at six timestamps.
- [x] T37 Service and menu bar 2026-09-27 evening (user: "integrate toolbar option with your suggested option"): the process is a menu bar service. Every pane is tapped by the poller and judged at once; the summary store sorts sessions and panes loudest first with snippets and writes `~/.local/state/watchglass/verdicts.json` atomically on change; the tray dot takes the top level's colour with the needs-you count as title; ⌃⌥W or a click opens a frameless panel (Enter go, 1…9, a/r answer, i idle, o board, Esc); the board is built on demand and destroyed on close; notifications moved to Rust with a 10 s per-subject limit; autostart in the menu. Proof: Rust tests for the store (order, counts once, write-on-change, dropped verdicts), the tray dot, and the grace timer; TS test for panel rows; panel screenshot against the mock; release build installed. Live 2026-09-27 19:46 on the installed 0.2.0: a fresh `wg-e2e` session with four windows was tapped within a poll, Jev judged failing 0.99 / working 1.0 / warning 1.0 and the shell went idle by rule, `verdicts.json` sorted them loudest first with top=failing, and killing a window then the session removed them and their tap files within 4 s. Not yet seen by me: the tray dot and the panel on screen, a real Approve.
- [ ] T38 Consumers of `verdicts.json`: a picker in claude-sessions.nvim, a tmux status segment.
- [ ] T33 Classifier debounce cap of 10 s is in; a global Jev budget for many panes is not.
- [ ] T30 Shell hook for processes outside tmux.
- [ ] T31 Replay a capture from the UI. Packaging is done: `pnpm tauri build --bundles app`.
