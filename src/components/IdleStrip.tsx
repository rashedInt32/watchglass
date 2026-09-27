import type { PaneInfo } from "../lib/ipc";

type Props = {
  panes: PaneInfo[];
  onExpand: (id: string) => void;
};

/** Panes the user hid, as chips; click one to bring it back to the grid. */
export function IdleStrip({ panes, onExpand }: Props) {
  if (panes.length === 0) return null;
  return (
    <footer className="strip">
      <span className="strip-label">hidden</span>
      {panes.map((p) => (
        <button key={p.id} className="chip" onClick={() => onExpand(p.id)} title={`${p.cwd} · click to show again`}>
          <i className="dot dot--idle" />
          {p.session}:{p.windowName}
          {p.paneIndex > 0 ? `.${p.paneIndex}` : ""}
          <span className="chip-cmd">{p.command}</span>
        </button>
      ))}
    </footer>
  );
}
