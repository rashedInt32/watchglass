# watchglass

Everything running in your tmux, in one window, with Jev telling you what
needs you first.

Every tmux pane becomes a live tile that mirrors the real pane, rendered by
Ghostty's terminal engine. Every Claude Code session is listed on the left.
Jev judges each pane and each session on a five-step ladder, and colour and
order follow: red needs you, orange is failing, amber is a warning, green
is working, grey is idle. No configuration. Nothing is started by the app.

## Demo

![watchglass demo: tmux panes as live tiles, Claude sessions on the left, Jev colours, approve, reply, jump, search, focus, new pane](docs/demo.gif)

Forty seconds, recorded against the app's scripted mock so the panes are
reproducible: [docs/demo.mp4](docs/demo.mp4). Re-record with `pnpm demo`
then `pnpm demo:encode` (needs Chrome and ffmpeg).

## Status

Early v0.2. Built against [SPEC.md](SPEC.md); progress in [TASKS.md](TASKS.md).
macOS first.

## Run it

Requirements: tmux, Node 20+, pnpm, Rust 1.90+ (pinned to 1.98.1 in
`rust-toolchain.toml`; rustup installs it on first build). Optional: a
TypeSafe key in `~/.config/typesafe/key` or `TYPESAFE_API_KEY` for Jev.

```sh
pnpm install
pnpm tauri dev
```

Open some panes in tmux and they appear. If you have a Ghostty config, the
tiles use its font and theme.

## Keys

| key | action |
| --- | --- |
| ⌘J | jump to the newest error line in any pane |
| ⌘K | search every pane's scrollback |
| ⌘1…9 | focus one pane full-window |
| Enter / Esc | enter or leave focus mode |
| j / k | move the active tile |
| n / p | next / previous marker in the active tile |
| f | back to live, or pause following |
| g | go to the active pane in tmux |
| i | type one line into the active pane |
| ⇧N | new pane: run a command in a new tmux window |
| s | toggle priority order |
| ? | show the keys |

## Acting without typing into a terminal

watchglass never becomes a terminal. It offers a few explicit actions
instead, all of them `tmux send-keys` underneath:

- a reply box on every Claude session in the sidebar
- Approve and Reject while a session waits for permission
- a one-line command box, Enter, and Ctrl-C on every tile
- ⇧N to run a command in a new tmux window, which shows up as a tile
- `g` to jump your tmux client to the pane for anything deeper

## How it watches

- `tmux list-panes -a` every 1.5 s finds panes.
- `tmux capture-pane -e -J` gives a pane's scrollback with colour; then
  `tmux pipe-pane` streams its output to a file the app tails. Pipes close
  when the app quits.
- Claude sessions come from `~/.claude/sessions` and each session's
  transcript.
- Jev gets the last 40 lines of a pane, or a session's status and last
  message, and answers one of: attention, failing, warning, working, idle.
  `waiting` and `busy` sessions are answered by rules without a call.
  Verdicts are appended to `verdicts.log` in the app data directory.
- A session waiting for permission gets Approve and Reject buttons in the
  sidebar. They send `1` or `Esc` to its pane, the menu keys Claude expects.
- Idle shells collapse into a strip under the grid. Click one to show it; a
  shell that starts a command comes back on its own.
- Tap files rotate at 4 MB, and a restart closes pipes a crash left behind.

## UI work without the Rust side

`pnpm dev` in a plain browser runs a scripted mock: six fake panes, three
fake Claude sessions, and cycling verdicts.

## Tests

```sh
pnpm test                    # vitest: classifier, follow, verdict ordering
pnpm typecheck
cd src-tauri && cargo test   # tmux parsing, escape stripping, transcripts, Jev
```

## Credits

Rendering is [ghostty-web](https://github.com/coder/ghostty-web) by Coder,
which wraps Ghostty's `libghostty-vt` engine for the browser. Judgments are
[Jev](https://typesafe.ai) by TypeSafe. The shell is [Tauri 2](https://v2.tauri.app).
