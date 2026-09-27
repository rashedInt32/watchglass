import type { JevStatus } from "../lib/ipc";

type Props = {
  panes: number;
  attention: number;
  failing: number;
  sortByPriority: boolean;
  onSort: (on: boolean) => void;
  columns: number;
  onColumns: (n: number) => void;
  jev: JevStatus | null;
  overlay: boolean;
  help: boolean;
  onToggleHelp: () => void;
  onNewPane: () => void;
};

function Glyph() {
  return (
    <svg className="brand-glyph" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <circle cx="6.2" cy="6.2" r="1.7" fill="currentColor" />
    </svg>
  );
}

function ColsIcon({ n }: { n: number }) {
  const gap = 1.5;
  const w = (14 - gap * (n - 1)) / n;
  return (
    <svg viewBox="0 0 14 14" width="14" height="14" aria-hidden="true">
      {Array.from({ length: n }, (_, i) => (
        <rect key={i} x={i * (w + gap)} y="1" width={w} height="12" rx="1.5" fill="currentColor" />
      ))}
    </svg>
  );
}

export function TopBar(p: Props) {
  return (
    <header className={p.overlay ? "bar bar--overlay" : "bar"} data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <Glyph />
        <span data-tauri-drag-region>watchglass</span>
      </div>

      <div className="pills">
        <span className="pill">
          <i className={p.panes > 0 ? "dot dot--working" : "dot"} />
          {p.panes} pane{p.panes === 1 ? "" : "s"}
        </span>
        {p.attention > 0 && (
          <span className="pill pill--attention">
            <i className="dot dot--attention" />
            {p.attention} need you
          </span>
        )}
        {p.failing > 0 && (
          <span className="pill pill--failing">
            <i className="dot dot--failing" />
            {p.failing} failing
          </span>
        )}
      </div>

      <button className="icon-btn icon-btn--quiet" onClick={p.onNewPane} title="New pane (n with ⇧)">
        +
      </button>

      <div className="bar-spacer" data-tauri-drag-region />

      <span className={p.jev?.enabled ? "pill pill--jev" : "pill pill--off"} title={p.jev?.reason ?? "checking"}>
        <i className={p.jev?.enabled ? "dot dot--jev" : "dot"} />
        {p.jev?.enabled ? "Jev on" : "Jev off"}
      </span>

      <div className="seg" role="group" aria-label="Order">
        <button className={p.sortByPriority ? "seg-btn seg-btn--text seg-btn--on" : "seg-btn seg-btn--text"} onClick={() => p.onSort(true)} title="Loudest first (s)">
          priority
        </button>
        <button className={!p.sortByPriority ? "seg-btn seg-btn--text seg-btn--on" : "seg-btn seg-btn--text"} onClick={() => p.onSort(false)} title="tmux order (s)">
          tmux
        </button>
      </div>

      <div className="seg" role="group" aria-label="Columns">
        {[1, 2, 3].map((n) => (
          <button
            key={n}
            className={n === p.columns ? "seg-btn seg-btn--on" : "seg-btn"}
            onClick={() => p.onColumns(n)}
            title={`${n} column${n > 1 ? "s" : ""}`}
          >
            <ColsIcon n={n} />
          </button>
        ))}
      </div>

      <button className={p.help ? "icon-btn icon-btn--on" : "icon-btn"} onClick={p.onToggleHelp} title="Keys (?)">
        ?
      </button>
    </header>
  );
}
