# watchglass — specification (v0.2)

> Status: living contract, rewritten 2026-09-27 after the pivot away from
> configured commands. Change it before changing behavior.

## 1. Problem

A developer keeps many things running in tmux all day: dev servers, test
watchers, type checkers, containers, and several Claude Code sessions.
Failures scroll past in panes they are not looking at. Claude sessions stop
and wait for an answer nobody notices. Finding "what needs me right now"
means cycling through every pane.

## 2. Goal

One window that shows everything already running in tmux, live, with no
configuration, and tells you what needs you first. Every pane is a tile
mirroring the real pane. Every Claude Code session is listed. Jev judges
each pane and each session, and colour and order follow that judgment.

## 3. Non-goals

- Starting processes. watchglass only watches what tmux already runs.
- Typing into panes beyond a few explicit actions (go there, send a key).
- Processes outside tmux. A shell hook for those may come later.
- Windows. macOS first, Linux best effort.
- Cloud, accounts, telemetry. Everything stays on disk.

## 4. Scenarios

1. **Three Claude sessions, one asks a question.** Its row turns red in the
   sidebar with the question as the snippet, its tile moves to the front,
   and a notification names the session. One key goes to that pane in tmux.
2. **Tests fail in a window you are not looking at.** The tile's header
   turns the failing colour within a few seconds of the output changing.
   ⌘J lands on the failing line.
3. **Everything is fine.** All tiles are green or grey, sorted by tmux
   order, and nothing interrupts you.

## 5. Architecture

```
┌──────────────── Rust core (src-tauri) ───────────────────────────┐
│ tmux.rs    list-panes poll (1.5 s) → panes channel               │
│            attach: capture-pane -e -J → first chunk               │
│                    pipe-pane → file → tail thread → chunks        │
│            focus (select-window/pane, switch-client), send-keys   │
│ claude.rs  ~/.claude/sessions/*.json + transcript tail → sessions │
│ jev.rs     debounced per-id jobs → one Choice → verdict channel   │
│ capture.rs wgcap files per attached pane, retention 10           │
└──────────────────────────────────────────────────────────────────┘
                     │ chunks · panes · sessions · verdicts
┌──────────────── Web view (src) ──────────────────────────────────┐
│ ClaudePane  sidebar, sorted by priority, snippet, go-to           │
│ PaneTile ×N ghostty-web mirror at the pane's cols×rows, font     │
│             scaled to fit; follow/pause, markers, jump, search    │
│ TopBar      counts, priority/tmux order, Jev status, columns     │
└──────────────────────────────────────────────────────────────────┘
```

### 5.1 Discovery and taps

`tmux list-panes -a` every 1.5 s. A changed list is pushed to the UI and
vanished panes are detached. Attaching a pane runs `capture-pane -p -e -J`
for the current scrollback, then `pipe-pane -O 'cat >> <tap file>'` and a
thread that tails the file every 80 ms. A few milliseconds between the two
may be lost, which beats showing them twice. Detaching closes the pipe and
removes the tap file. All pipes close when the window is destroyed.

`tmux` is resolved from `PATH`, then the Homebrew, local, and MacPorts
prefixes, because GUI apps see a bare `PATH`.

### 5.2 Mirroring

A tile's terminal has the pane's real column and row count and never
reflows. The font size is the largest that fits the tile, between 5 and
22 px, so a grid tile is a legible miniature and focus mode is readable.
Pane resizes in tmux resize the mirror. Rendering, Ghostty theme and font
import, follow and pause, markers, jump, and search carry over from v0.1.

### 5.3 Claude sessions

Each file in `~/.claude/sessions` gives the session id, pid, name, cwd,
status, timestamps, and the tmux pane. Dead pids are dropped. The
transcript at `~/.claude/projects/<cwd with / and . as ->/<id>.jsonl` gives
the last assistant message that carried text, read from the file's tail.
Polled every 2 s and pushed on change.

### 5.4 Judgment

One Jev Choice per subject with five levels, in priority order:
`attention` (stopped and waiting for a person), `failing`, `warning`,
`working`, `idle`.

- **Panes**: state is the window, foreground command, title, cwd, and the
  last 40 non-empty lines of plain text (escapes stripped, carriage returns
  resolved). Jobs are debounced 2.5 s per pane and skipped when the tail is
  unchanged.
- **Claude sessions**: state is status, seconds since activity, cwd, and the
  last message. Rules answer first: `waiting` is `attention`, `busy` is
  `working`. Only `idle` and unknown statuses go to Jev.
- **Shells**: a pane whose foreground command is an interactive shell is
  `idle` by rule and never sent to Jev. Its scrollback may show a finished
  program's last screen, which reads like a prompt to a model.
- A session's verdict outranks the generic verdict of its pane.
- Verdicts carry level, confidence, probabilities, and source (`jev` or
  `rule`). Each is appended to `verdicts.log` in the app data dir.
- The key comes from `TYPESAFE_API_KEY` or `~/.config/typesafe/key`. Without
  a key, rules still run and everything else shows as idle.

### 5.5 Presentation and actions

Colour ladder: attention red with a pulse, failing orange, warning amber,
working green, idle grey. Tile headers tint with their level. Priority
order sorts loudest first; tmux order is the alternative, toggled with `s`.
Every pane is a tile until the user hides it (`h`); hidden panes sit in a
strip under the grid and come back on click. The app never hides a pane
on its own.
Notifications fire only when a subject rises into failing or attention and
its tile is not the active visible one, rate limited per subject.

Keys: ⌘J newest error line, ⌘K search, ⌘1…9 focus a tile, Enter/Esc focus
mode, j/k active tile, n/p markers, f follow, g go to the pane in tmux,
h hide the pane, i one-line input, ⇧N new pane, s order, ? keys.

### 5.6 Actions, not a terminal

watchglass does not emulate keyboard input. It offers explicit actions,
each one a `tmux send-keys`: Approve (`1`) and Reject (`Escape`) while a
session waits, in the panel and on the board; a one-line command box,
Enter, and Ctrl-C per tile; and ⇧N to open a new tmux window running a
command, which the poller picks up as a tile. A free-text reply box was
tried and removed: it invited typing, and the tile input covers the rare
need. For anything deeper, `g` or Enter in the panel moves the tmux client
to the pane.

### 5.7 Service and menu bar

The process is a menu bar service; windows are views on it. Discovery,
taps, and judgment run from launch with no window open: the poller taps
every pane it finds, so a pane is judged on its scrollback the moment it
appears and on every change after. The core keeps one picture, the
summary: sessions and panes with their level, source, confidence, and a
snippet (a session's last assistant text, a pane's last line), sorted
loudest first, with per-level counts in which a session and its own pane
count once.

The menu bar item is a dot in the loudest level's colour, with the number
of attention and failing items as its title. A left click or ⌃⌥W toggles
the panel: a frameless window under the item listing the summary, with
Enter, 1…9, a, r, i, o, and Esc as its keys. It hides when it loses focus.
The menu offers Show list, Open the board, Start at login, and Quit. The
board is created on demand from its window config and destroyed on close;
the Dock icon exists only while it is open. Closing every window never
quits; only Quit does.

Notifications belong to the service: a verdict rising into failing or
attention, rate limited per subject, skipped while the board has focus.

The summary is written to `$XDG_STATE_HOME/watchglass/verdicts.json`
(default `~/.local/state/watchglass/verdicts.json`), atomically and only
when it changed. It is the contract for other consumers, such as a Neovim
picker or a tmux status line.

## 6. IPC contract

Commands: `subscribe(on_chunk, on_panes, on_claude, on_verdict)` for the
board, `subscribe_summary(on_summary)` and `summary` for the panel,
`tmux_available`, `tmux_status`, `list_panes`, `attach_pane(id)`,
`detach_pane(id)`, `focus_pane(id)`, `send_keys(id, keys[])`,
`send_line(id, text)`, `new_window(session, name, command)`,
`claude_sessions`, `ghostty_appearance`, `jev_status`, `open_main`,
`hide_panel`, `verdicts_path`. A subscription is per window label and
replaced on reload; `attach_pane` returns the pane's screen while the tap
itself belongs to the poller.

```ts
type ChunkMsg   = { id; seq; t; data /* base64 */ };
type PaneInfo   = { id; session; windowIndex; windowName; paneIndex; pid; command; title; cols; rows; cwd; active; attached };
type ClaudeSession = { sessionId; pid; name; cwd; status; statusUpdatedAt; updatedAt; tmux; paneId; lastText; lastTextAt };
type VerdictMsg = { id; kind: "pane" | "claude"; level; confidence; probabilities; source: "jev" | "rule"; at };
type SessionRow = { sessionId; name; cwd; status; tmux; paneId; level; confidence; source; snippet; updatedAt };
type PaneRow    = { id; session; windowIndex; windowName; paneIndex; command; title; cwd; level; confidence; source; snippet };
type Summary    = { updatedAt; top: Level; counts: Partial<Record<Level, number>>; sessions: SessionRow[]; panes: PaneRow[] };
```

`Summary` is also the schema of `verdicts.json`.

## 7. Acceptance criteria

1. With five tmux panes open, five tiles appear within two seconds and
   show each pane's current scrollback, with colour.
2. Output typed into any pane appears in its tile within 200 ms.
3. A Claude session in `waiting` shows red in the sidebar with no Jev call.
4. A Claude session that ends with a question turns `attention` via Jev and
   notifies when its tile is not active.
5. A pane with failing tests turns `failing` within a few seconds.
6. Priority order puts attention and failing tiles first; `s` toggles.
7. `g` moves the tmux client to the active pane.
8. Closing a pane removes its tile and its pipe; quitting the app closes
   every pipe.
9. Unit tests cover pane parsing, escape stripping, transcript parsing,
   question building, verdict parsing, rules, and level ordering.
10. With no window open, a pane that starts failing turns the menu bar dot
    orange and updates `verdicts.json` within a few seconds.
11. ⌃⌥W shows the panel with the same order the board uses; Enter on a row
    moves the tmux client there and hides the panel.
12. Closing the board leaves the service, the taps, and the file alive.

## 8. Risks

- `pipe-pane` allows one pipe per pane; a user's own pipe would be replaced.
- Large panes mirror at tiny fonts in a dense grid; focus mode is the
  answer, and a per-tile zoom may follow.
- Jev cost scales with output churn; the debounce and unchanged-tail skip
  bound it. Busy panes classify at most every 2.5 s.
- Sessions outside tmux are listed but cannot be mirrored.
