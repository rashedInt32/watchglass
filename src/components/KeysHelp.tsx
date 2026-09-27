const KEYS: [string, string][] = [
  ["⌘J", "Jump to the newest error line in any pane"],
  ["⌘K", "Search every pane's scrollback"],
  ["⌘1…9", "Focus one pane full-window"],
  ["Enter / Esc", "Enter or leave focus mode"],
  ["j / k", "Move the active tile"],
  ["n / p", "Next / previous marker in the active tile"],
  ["f", "Back to live, or pause following"],
  ["g", "Go to the active pane in tmux"],
  ["i", "Type one line into the active pane"],
  ["⇧N", "New pane: run a command in a new tmux window"],
  ["s", "Toggle priority order"],
  ["?", "This list"],
];

export function KeysHelp({ onClose }: { onClose: () => void }) {
  return (
    <div className="help-backdrop" onMouseDown={onClose}>
      <div className="help" onMouseDown={(e) => e.stopPropagation()}>
        <div className="help-title">Keys</div>
        <dl className="help-list">
          {KEYS.map(([key, what]) => (
            <div className="help-row" key={key}>
              <dt>
                <kbd>{key}</kbd>
              </dt>
              <dd>{what}</dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  );
}
