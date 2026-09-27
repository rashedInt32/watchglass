type Props = { tmux: boolean | null; panes: number };

export function EmptyState({ tmux, panes }: Props) {
  const title = tmux === false ? "tmux is not reachable" : panes === 0 ? "No tmux panes yet" : "Loading";
  const body =
    tmux === false
      ? "watchglass watches what already runs in your tmux. Start a tmux session, or make sure the tmux binary is on the PATH the app sees."
      : "Open something in tmux and it appears here as a live tile. Claude Code sessions show on the left.";
  return (
    <div className="empty">
      <div className="empty-card">
        <div className="empty-glyph" aria-hidden="true">
          <svg viewBox="0 0 48 48" width="40" height="40">
            <circle cx="24" cy="24" r="17" fill="none" stroke="currentColor" strokeWidth="2.5" />
            <circle cx="18.5" cy="18.5" r="4.5" fill="currentColor" />
          </svg>
        </div>
        <h1>{title}</h1>
        <p>{body}</p>
      </div>
    </div>
  );
}
