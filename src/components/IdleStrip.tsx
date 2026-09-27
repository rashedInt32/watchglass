import type { PaneInfo } from "../lib/ipc";

type Props = {
  panes: PaneInfo[];
  onExpand: (id: string) => void;
};

/** Idle shells, collapsed to chips so the grid keeps its room for real work. */
export function IdleStrip({ panes, onExpand }: Props) {
  if (panes.length === 0) return null;
  return (
    <footer className="strip">
      <span className="strip-label">idle shells</span>
      {panes.map((p) => (
        <button key={p.id} className="chip" onClick={() => onExpand(p.id)} title={`${p.cwd} · click to show`}>
          <i className="dot dot--idle" />
          {p.session}:{p.windowName}
          {p.paneIndex > 0 ? `.${p.paneIndex}` : ""}
          <span className="chip-cmd">{p.command}</span>
        </button>
      ))}
    </footer>
  );
}
