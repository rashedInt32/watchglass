import { describe as group, expect, it } from "vitest";
import { comparePriority, describe, rank, shouldNotify, tmuxOrder } from "./verdict";

group("levels", () => {
  it("rank attention highest and undefined lowest", () => {
    expect(rank("attention")).toBeGreaterThan(rank("failing"));
    expect(rank("failing")).toBeGreaterThan(rank("warning"));
    expect(rank("warning")).toBeGreaterThan(rank("working"));
    expect(rank("working")).toBeGreaterThan(rank("idle"));
    expect(rank(undefined)).toBe(0);
  });

  it("sort loudest first", () => {
    const levels = ["idle", "attention", "working", "failing"] as const;
    const sorted = [...levels].sort(comparePriority);
    expect(sorted).toEqual(["attention", "failing", "working", "idle"]);
  });

  it("notify only when rising into failing or attention", () => {
    expect(shouldNotify(undefined, "attention")).toBe(true);
    expect(shouldNotify("working", "failing")).toBe(true);
    expect(shouldNotify("failing", "failing")).toBe(false);
    expect(shouldNotify("attention", "failing")).toBe(false);
    expect(shouldNotify("idle", "warning")).toBe(false);
  });

  it("describes verdicts with confidence for jev only", () => {
    expect(describe(undefined)).toBe("");
    expect(describe({ id: "x", kind: "pane", level: "failing", confidence: 0.874, probabilities: {}, source: "jev", at: 0 })).toBe("failing · 87%");
    expect(describe({ id: "x", kind: "claude", level: "attention", confidence: 1, probabilities: {}, source: "rule", at: 0 })).toBe("needs you");
  });

  it("orders panes like tmux", () => {
    const a = { session: "main", windowIndex: 2, paneIndex: 0 };
    const b = { session: "main", windowIndex: 1, paneIndex: 3 };
    const c = { session: "alpha", windowIndex: 9, paneIndex: 0 };
    expect([a, b, c].sort(tmuxOrder)).toEqual([c, b, a]);
  });
});
