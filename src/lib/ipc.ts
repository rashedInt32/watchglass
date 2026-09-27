// Typed wrappers over the Tauri commands declared in src-tauri/src/lib.rs.
import { Channel, invoke } from "@tauri-apps/api/core";

export type ChunkMsg = { id: string; seq: number; t: number; data: string };

export type PaneInfo = {
  id: string;
  session: string;
  windowIndex: number;
  windowName: string;
  paneIndex: number;
  pid: number;
  command: string;
  title: string;
  cols: number;
  rows: number;
  cwd: string;
  active: boolean;
  attached: boolean;
  piped: boolean;
};

/** Shells at a prompt; collapsed when idle so real work gets the space. */
export const SHELLS = new Set(["zsh", "bash", "fish", "sh", "nu", "dash", "ksh", "tcsh"]);

export type ClaudeSession = {
  sessionId: string;
  pid: number;
  name: string;
  cwd: string;
  status: string;
  statusUpdatedAt: number;
  updatedAt: number;
  tmux: string | null;
  paneId: string | null;
  lastText: string | null;
  lastTextAt: string | null;
};

export type Level = "idle" | "working" | "warning" | "failing" | "attention";

export type VerdictMsg = {
  id: string;
  kind: "pane" | "claude";
  level: Level;
  confidence: number;
  probabilities: Record<string, number>;
  source: "jev" | "rule";
  at: number;
};

/** `snapshot` is the pane's scrollback at attach time, base64; write it before any live chunk. */
export type AttachInfo = { id: string; cols: number; rows: number; snapshot: string };

export type JevStatus = { enabled: boolean; reason: string };

export type TmuxStatus = { available: boolean; path: string; socket: string; error: string | null };

export const inTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type Subscriber = {
  onChunk: (msg: ChunkMsg) => void;
  onPanes: (panes: PaneInfo[]) => void;
  onClaude: (sessions: ClaudeSession[]) => void;
  onVerdict: (verdict: VerdictMsg) => void;
};

export async function subscribe(s: Subscriber): Promise<void> {
  const onChunk = new Channel<ChunkMsg>();
  onChunk.onmessage = s.onChunk;
  const onPanes = new Channel<PaneInfo[]>();
  onPanes.onmessage = s.onPanes;
  const onClaude = new Channel<ClaudeSession[]>();
  onClaude.onmessage = s.onClaude;
  const onVerdict = new Channel<VerdictMsg>();
  onVerdict.onmessage = s.onVerdict;
  await invoke("subscribe", { onChunk, onPanes, onClaude, onVerdict });
}

export const tmuxAvailable = () => invoke<boolean>("tmux_available");
export const tmuxStatus = () => invoke<TmuxStatus>("tmux_status");
export const listPanes = () => invoke<PaneInfo[]>("list_panes");
export const attachPane = (id: string) => invoke<AttachInfo>("attach_pane", { id });
export const detachPane = (id: string) => invoke<boolean>("detach_pane", { id });
export const focusPane = (id: string) => invoke<void>("focus_pane", { id });
export const sendKeys = (id: string, keys: string[]) => invoke<void>("send_keys", { id, keys });
export const sendLine = (id: string, text: string) => invoke<void>("send_line", { id, text });
export const newWindow = (session: string, name: string, command: string) =>
  invoke<string>("new_window", { session, name, command });
export const claudeSessions = () => invoke<ClaudeSession[]>("claude_sessions");
export const jevStatus = () => invoke<JevStatus>("jev_status");

/** base64 → bytes. Good enough for log volumes; see SPEC for the plan. */
export function decodeChunk(data: string): Uint8Array {
  const bin = atob(data);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
