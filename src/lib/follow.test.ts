import { describe, expect, it } from "vitest";
import { reduceFollow, reduceUnread } from "./follow";

describe("reduceFollow", () => {
  it("pauses on scrolling up and resumes at the bottom", () => {
    expect(reduceFollow("live", { type: "scroll", atBottom: false })).toBe("paused");
    expect(reduceFollow("paused", { type: "scroll", atBottom: true })).toBe("live");
    expect(reduceFollow("live", { type: "scroll", atBottom: true })).toBe("live");
  });

  it("pauses on any jump and resumes on live", () => {
    expect(reduceFollow("live", { type: "jump" })).toBe("paused");
    expect(reduceFollow("paused", { type: "jump" })).toBe("paused");
    expect(reduceFollow("paused", { type: "live" })).toBe("live");
  });

  it("toggles", () => {
    expect(reduceFollow("live", { type: "toggle" })).toBe("paused");
    expect(reduceFollow("paused", { type: "toggle" })).toBe("live");
  });
});

describe("reduceUnread", () => {
  it("counts errors until focus clears them", () => {
    let n = 0;
    n = reduceUnread(n, "error");
    n = reduceUnread(n, "error");
    expect(n).toBe(2);
    expect(reduceUnread(n, "focus")).toBe(0);
  });
});
