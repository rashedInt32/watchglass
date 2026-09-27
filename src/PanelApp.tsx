// The menu bar panel: everything the service watches, loudest first, with
// the one-key answers. Lives in its own small window; the board is a click
// away.
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import "./App.css";
import "./panel.css";
import { chromeVars, fontFamilyCss, toTerminalTheme, type Appearance } from "./lib/appearance";
import { backend } from "./lib/backend";
import type { Level, PaneRow, SessionRow, Summary } from "./lib/ipc";
import { LEVEL_WORD } from "./lib/verdict";

type Row =
  | { kind: "claude"; id: string; level: Level; paneId: string | null; session: SessionRow }
  | { kind: "pane"; id: string; level: Level; paneId: string; pane: PaneRow };

const EMPTY: Summary = { updatedAt: 0, top: "idle", counts: {}, sessions: [], panes: [] };

function basename(path: string): string {
  const clean = path.replace(/\/+$/, "");
  return clean.split("/").pop() || clean;
}

/** Sessions first, then panes not owned by a session; idle panes go last, behind a toggle. */
export function toRows(s: Summary): { live: Row[]; idle: Row[] } {
  const owned = new Set(s.sessions.map((x) => x.paneId).filter((x): x is string => !!x));
  const claude: Row[] = s.sessions.map((x) => ({ kind: "claude", id: x.sessionId, level: x.level, paneId: x.paneId, session: x }));
  const panes: Row[] = s.panes
    .filter((p) => !owned.has(p.id))
    .map((p) => ({ kind: "pane", id: p.id, level: p.level, paneId: p.id, pane: p }));
  return {
    live: [...claude, ...panes.filter((r) => r.level !== "idle")],
    idle: panes.filter((r) => r.level === "idle"),
  };
}

export default function PanelApp() {
  const [summary, setSummary] = useState<Summary>(EMPTY);
  const [selected, setSelected] = useState<string | null>(null);
  const [showIdle, setShowIdle] = useState(false);
  const [path, setPath] = useState("");
  const [appearance, setAppearance] = useState<Appearance | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.body.classList.add("panel-body");
    void backend.subscribeSummary(setSummary).catch(() => setSummary(EMPTY));
    void backend.verdictsPath().then(setPath).catch(() => {});
    void backend.ghosttyAppearance().then(setAppearance).catch(() => {});
  }, []);

  // Same colours as the board: the user's Ghostty theme when it can be read.
  const rootStyle = useMemo(
    () => chromeVars(toTerminalTheme(appearance), fontFamilyCss(appearance)) as CSSProperties,
    [appearance],
  );

  const { live, idle } = useMemo(() => toRows(summary), [summary]);
  const visible = useMemo(() => (showIdle ? [...live, ...idle] : live), [live, idle, showIdle]);
  const where = useMemo(() => new Map(summary.panes.map((p) => [p.id, `${p.session}:${p.windowName}`])), [summary]);
  const selIndex = Math.max(0, visible.findIndex((r) => r.id === selected));
  const current = visible[selIndex];

  const go = useCallback((r: Row | undefined) => {
    if (r?.paneId) void backend.focusPane(r.paneId);
  }, []);
  const answer = useCallback((r: Row | undefined, a: "approve" | "reject") => {
    if (r?.kind !== "claude" || !r.paneId || r.session.status !== "waiting") return;
    void backend.sendKeys(r.paneId, [a === "approve" ? "1" : "Escape"]);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const move = (d: number) => {
        const n = visible.length;
        if (n) setSelected(visible[(selIndex + d + n) % n]!.id);
      };
      switch (e.key) {
        case "j":
        case "ArrowDown":
          move(1);
          break;
        case "k":
        case "ArrowUp":
          move(-1);
          break;
        case "Enter":
          go(current);
          break;
        case "a":
          answer(current, "approve");
          break;
        case "r":
          answer(current, "reject");
          break;
        case "o":
          void backend.openMain();
          break;
        case "i":
          setShowIdle((v) => !v);
          break;
        case "Escape":
          void backend.hidePanel();
          break;
        default:
          if (/^[1-9]$/.test(e.key)) go(visible[Number(e.key) - 1]);
          else return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible, selIndex, current, go, answer]);

  useEffect(() => {
    listRef.current?.querySelector(".prow--selected")?.scrollIntoView({ block: "nearest" });
  }, [selIndex, visible]);

  const needs = (summary.counts.attention ?? 0) + (summary.counts.failing ?? 0);
  const claudeCount = summary.sessions.length;
  const paneRows = visible.filter((r) => r.kind === "pane");
  let index = 0;
  const rowView = (r: Row) => {
    const i = index++;
    return (
      <RowView
        key={r.id}
        row={r}
        index={i}
        selected={i === selIndex}
        where={r.kind === "claude" ? (r.paneId ? where.get(r.paneId) ?? r.paneId : "not in tmux") : undefined}
        onGo={() => go(r)}
        onAnswer={(a) => answer(r, a)}
        onSelect={() => setSelected(r.id)}
      />
    );
  };

  return (
    <div className="app panel" style={rootStyle}>
      <header className="panel-head">
        <i className={`dot dot--${summary.top}`} />
        <span className="panel-title">watchglass</span>
        {needs > 0 ? (
          <span className={`panel-pill panel-pill--${summary.top}`}>{needs} need you</span>
        ) : (
          <span className="panel-quiet">{summary.updatedAt ? "all quiet" : "starting…"}</span>
        )}
        <button className="panel-btn" title="Open the board (o)" onClick={() => void backend.openMain()}>
          board
        </button>
      </header>
      <div className="panel-list" ref={listRef}>
        <div className="panel-section">
          Claude <span className="panel-section-n">{claudeCount}</span>
        </div>
        {claudeCount === 0 && <div className="panel-empty">No Claude Code sessions running.</div>}
        {visible.filter((r) => r.kind === "claude").map(rowView)}
        <div className="panel-section">
          Panes <span className="panel-section-n">{paneRows.length + (showIdle ? 0 : idle.length)}</span>
        </div>
        {paneRows.length === 0 && idle.length === 0 && (
          <div className="panel-empty">No tmux panes. Start tmux and they appear here.</div>
        )}
        {paneRows.map(rowView)}
        {idle.length > 0 && (
          <button className="panel-idle" onClick={() => setShowIdle((v) => !v)} title="Toggle idle panes (i)">
            {showIdle ? "▾" : "▸"} {idle.length} idle
          </button>
        )}
      </div>
      <footer className="panel-foot" title={path ? `Verdicts file: ${path}` : undefined}>
        <span>↑↓ move</span>
        <span>⏎ go</span>
        <span>a approve</span>
        <span>r reject</span>
        <span>o board</span>
        <span>esc</span>
      </footer>
    </div>
  );
}

type RowProps = {
  row: Row;
  index: number;
  selected: boolean;
  where?: string;
  onGo: () => void;
  onAnswer: (a: "approve" | "reject") => void;
  onSelect: () => void;
};

function RowView({ row, index, selected, where, onGo, onAnswer, onSelect }: RowProps) {
  const level = row.level;
  const title = row.kind === "claude" ? basename(row.session.cwd) : `${row.pane.session}:${row.pane.windowName}`;
  const meta = row.kind === "claude" ? row.session.status : row.pane.command;
  const snippet = row.kind === "claude" ? row.session.snippet : row.pane.snippet;
  const canAnswer = row.kind === "claude" && row.session.status === "waiting" && !!row.paneId;
  const canGo = !!row.paneId;
  return (
    <div
      className={`prow prow--${level}${selected ? " prow--selected" : ""}${canGo ? "" : " prow--nopane"}`}
      onMouseEnter={onSelect}
      onClick={onGo}
      title={canGo ? "Go to this pane in tmux" : "Not running in tmux"}
    >
      <div className="prow-top">
        <i className={`dot dot--${level}`} />
        <span className="prow-title">{title}</span>
        <span className="prow-meta">{meta}</span>
        {where && <span className="prow-where">{where}</span>}
        <span className={`prow-level prow-level--${level}`}>{LEVEL_WORD[level]}</span>
        <kbd className="prow-index">{index < 9 ? index + 1 : ""}</kbd>
      </div>
      {snippet && <div className={`prow-snippet${row.kind === "pane" ? " prow-snippet--mono" : ""}`}>{snippet}</div>}
      {canAnswer && (
        <div className="prow-actions">
          <button
            className="btn btn--small btn--primary"
            title="Send 1 to the pane: the affirmative menu option (a)"
            onClick={(e) => {
              e.stopPropagation();
              onAnswer("approve");
            }}
          >
            Approve
          </button>
          <button
            className="btn btn--small"
            title="Send Esc to the pane: cancels the prompt (r)"
            onClick={(e) => {
              e.stopPropagation();
              onAnswer("reject");
            }}
          >
            Reject
          </button>
          <span className="prow-actions-hint">waiting for permission</span>
        </div>
      )}
    </div>
  );
}
