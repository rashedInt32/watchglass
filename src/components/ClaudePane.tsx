import type { ClaudeSession, VerdictMsg } from "../lib/ipc";
import { comparePriority, describe, LEVEL_WORD } from "../lib/verdict";

type Props = {
  sessions: ClaudeSession[];
  verdicts: Record<string, VerdictMsg>;
  activePaneId: string | null;
  onSelect: (session: ClaudeSession) => void;
  onGo: (session: ClaudeSession) => void;
  /** Answer a permission prompt: "1" approves, Esc rejects, as Claude's menu expects. */
  onAnswer: (session: ClaudeSession, answer: "approve" | "reject") => void;
};

function age(ms: number): string {
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  return `${Math.round(m / 60)}h`;
}

function basename(path: string): string {
  const clean = path.replace(/\/+$/, "");
  return clean.split("/").pop() || clean;
}

export function ClaudePane({ sessions, verdicts, activePaneId, onSelect, onGo, onAnswer }: Props) {
  const rows = [...sessions].sort(
    (a, b) => comparePriority(verdicts[a.sessionId]?.level, verdicts[b.sessionId]?.level) || b.updatedAt - a.updatedAt,
  );
  const counts = rows.reduce<Record<string, number>>((acc, s) => {
    const level = verdicts[s.sessionId]?.level ?? "idle";
    acc[level] = (acc[level] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <aside className="claude">
      <header className="claude-head">
        <span className="claude-title">Claude</span>
        <span className="claude-count">{rows.length}</span>
        {counts.attention ? <span className="claude-pill claude-pill--attention">{counts.attention} need you</span> : null}
        {counts.failing ? <span className="claude-pill claude-pill--failing">{counts.failing} failing</span> : null}
      </header>
      {rows.length === 0 && <div className="claude-empty">No Claude Code sessions running.</div>}
      <ul className="claude-list">
        {rows.map((s) => {
          const v = verdicts[s.sessionId];
          const level = v?.level ?? "idle";
          const isActive = !!s.paneId && s.paneId === activePaneId;
          const canAnswer = s.status === "waiting" && !!s.paneId;
          return (
            <li
              key={s.sessionId}
              className={`claude-row claude-row--${level}${isActive ? " claude-row--active" : ""}${s.paneId ? "" : " claude-row--nopane"}`}
              onClick={() => onSelect(s)}
              title={s.cwd}
            >
              <div className="claude-row-top">
                <i className={`dot dot--${level}`} />
                <span className="claude-name">{basename(s.cwd)}</span>
                <span className="claude-status">{s.status}</span>
                <span className="claude-age">{age(s.updatedAt)}</span>
                <span className={`claude-level claude-level--${level}`}>{v ? describe(v) : LEVEL_WORD.idle}</span>
                <button
                  className="tile-btn"
                  title={s.paneId ? "Go to this session in tmux" : "Not running in tmux"}
                  disabled={!s.paneId}
                  onClick={(e) => {
                    e.stopPropagation();
                    onGo(s);
                  }}
                >
                  ↗
                </button>
              </div>
              {s.lastText && <div className="claude-text">{s.lastText}</div>}
              {canAnswer && (
                <div className="claude-actions">
                  <button
                    className="btn btn--small btn--primary"
                    title="Send 1 to the pane: the affirmative menu option"
                    onClick={(e) => {
                      e.stopPropagation();
                      onAnswer(s, "approve");
                    }}
                  >
                    Approve
                  </button>
                  <button
                    className="btn btn--small"
                    title="Send Esc to the pane: cancels the prompt"
                    onClick={(e) => {
                      e.stopPropagation();
                      onAnswer(s, "reject");
                    }}
                  >
                    Reject
                  </button>
                  <span className="claude-actions-hint">waiting for permission</span>
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
