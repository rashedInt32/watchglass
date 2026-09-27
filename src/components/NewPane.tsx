import { useEffect, useRef, useState } from "react";

type Props = {
  sessions: string[];
  onCreate: (session: string, name: string, command: string) => Promise<void>;
  onClose: () => void;
};

/** Opens a new tmux window running a command; it shows up as a tile on its own. */
export function NewPane({ sessions, onCreate, onClose }: Props) {
  const [session, setSession] = useState(sessions[0] ?? "");
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const commandRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    commandRef.current?.focus();
  }, []);

  async function submit() {
    if (!command.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onCreate(session, name.trim(), command.trim());
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <div className="dialog-backdrop" onMouseDown={onClose}>
      <div className="dialog dialog--small" onMouseDown={(e) => e.stopPropagation()} role="dialog" aria-label="New pane">
        <header className="dialog-head">
          <h2>New pane</h2>
          <p>Runs in a new tmux window and appears here as a tile. The window stays open after the command ends.</p>
        </header>
        <div className="newpane-form">
          <label className="newpane-field">
            <span>session</span>
            <select value={session} onChange={(e) => setSession(e.target.value)}>
              {sessions.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </label>
          <label className="newpane-field">
            <span>name</span>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="optional" spellCheck={false} />
          </label>
          <label className="newpane-field newpane-field--wide">
            <span>command</span>
            <input
              ref={commandRef}
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="pnpm dev"
              spellCheck={false}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") void submit();
                if (e.key === "Escape") onClose();
              }}
            />
          </label>
        </div>
        {error && <div className="banner banner--error">{error}</div>}
        <footer className="dialog-foot">
          <span className="setup-count" />
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--primary" disabled={!command.trim() || busy} onClick={() => void submit()}>
            Open
          </button>
        </footer>
      </div>
    </div>
  );
}
