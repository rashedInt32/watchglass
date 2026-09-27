// Priority levels as Jev returns them, ordered, with the UI words and colours.
import type { Level, VerdictMsg } from "./ipc";

export const LEVELS: Level[] = ["idle", "working", "warning", "failing", "attention"];

export function rank(level: Level | undefined): number {
  return level ? LEVELS.indexOf(level) : 0;
}

/** Highest priority first; ties keep input order when used with a stable sort. */
export function comparePriority(a: Level | undefined, b: Level | undefined): number {
  return rank(b) - rank(a);
}

export const LEVEL_WORD: Record<Level, string> = {
  attention: "needs you",
  failing: "failing",
  warning: "warning",
  working: "working",
  idle: "idle",
};

/** True when a change should interrupt the user: rising into failing or attention. */
export function shouldNotify(prev: Level | undefined, next: Level): boolean {
  return rank(next) >= rank("failing") && rank(next) > rank(prev);
}

export function describe(v: VerdictMsg | undefined): string {
  if (!v) return "";
  const word = LEVEL_WORD[v.level];
  return v.source === "rule" ? word : `${word} · ${Math.round(v.confidence * 100)}%`;
}

/** Ordinal for a pane in tmux order: session, window, pane. */
export function tmuxOrder(a: { session: string; windowIndex: number; paneIndex: number }, b: typeof a): number {
  return a.session.localeCompare(b.session) || a.windowIndex - b.windowIndex || a.paneIndex - b.paneIndex;
}
