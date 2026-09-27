import { describe, expect, it } from "vitest";
import type { Summary } from "./lib/ipc";
import { toRows } from "./PanelApp";

const summary: Summary = {
  updatedAt: 1,
  top: "attention",
  counts: { attention: 1, failing: 1, idle: 1 },
  sessions: [
    { sessionId: "s1", name: "a", cwd: "/x/docs", status: "waiting", tmux: null, paneId: "%5", level: "attention", confidence: 1, source: "rule", snippet: "May I?", updatedAt: 9 },
  ],
  panes: [
    { id: "%5", session: "main", windowIndex: 1, windowName: "claude", paneIndex: 0, command: "claude", title: "", cwd: "/x", level: "attention", confidence: 1, source: "rule", snippet: "" },
    { id: "%2", session: "shop", windowIndex: 2, windowName: "tests", paneIndex: 0, command: "node", title: "", cwd: "/x", level: "failing", confidence: 0.9, source: "jev", snippet: "FAIL" },
    { id: "%7", session: "nvim", windowIndex: 1, windowName: "zsh", paneIndex: 0, command: "zsh", title: "", cwd: "/x", level: "idle", confidence: 1, source: "rule", snippet: "" },
  ],
};

describe("toRows", () => {
  it("lists sessions first, skips panes a session owns, and parks idle panes", () => {
    const { live, idle } = toRows(summary);
    expect(live.map((r) => r.id)).toEqual(["s1", "%2"]);
    expect(idle.map((r) => r.id)).toEqual(["%7"]);
    expect(live[0]!.paneId).toBe("%5");
  });
});
