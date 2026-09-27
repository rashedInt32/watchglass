import { describe, expect, it } from "vitest";
import { classify, compilePatterns, MarkerCollector, parsePattern } from "./detect";

describe("classify with defaults", () => {
  const errors = [
    "ERR! code ELIFECYCLE",
    " ELIFECYCLE  Command failed with exit code 1.",
    "src/app.ts(12,5): error TS2322: Type 'string' is not assignable to type 'number'.",
    " FAIL  src/lib/detect.test.ts > classify",
    "   × renders the dashboard",
    "Error: connect ECONNREFUSED 127.0.0.1:5432",
    "AssertionError: expected 1 to be 2",
    "Traceback (most recent call last):",
    "thread 'main' panicked at src/main.rs:4:5",
    "Unhandled Rejection at: Promise",
    "Error: listen EADDRINUSE: address already in use :::3000",
    "web-1  | TypeError: Cannot read properties of undefined (reading 'map')",
    "✖ 1 problem (1 error, 0 warnings)",
  ];
  it.each(errors)("flags %s as error", (line) => {
    expect(classify(line)).toBe("error");
  });

  const warns = [
    "WARN  deprecated core-js@2.6.12: core-js@<3.23.3 is no longer maintained",
    " ⚠ Fast Refresh had to perform a full reload",
    "warning: unused variable `x`",
    "(node:1234) [DEP0040] DeprecationWarning: The `punycode` module is deprecated.",
  ];
  it.each(warns)("flags %s as warn", (line) => {
    expect(classify(line)).toBe("warn");
  });

  const clean = [
    "",
    "   ",
    "▲ Next.js 15.3.1",
    "- Local:        http://localhost:3000",
    "✓ Ready in 812ms",
    " Test Files  3 passed (3)",
    "      Tests  12 passed (12)",
    "Found 0 errors. Watching for file changes.",
    "[nodemon] restarting due to changes...",
    "GET /api/users 200 in 43ms",
    "Errorboundary rendered",
    "No errors found",
    "web-1  | ready - started server on 0.0.0.0:3000",
  ];
  it.each(clean)("leaves %j alone", (line) => {
    expect(classify(line)).toBeNull();
  });

  it("ignore patterns win over error patterns", () => {
    expect(classify("Tests: 0 failed, 12 passed")).toBeNull();
  });

  it("zero summaries do not hide a positive count on the same line", () => {
    expect(classify("Tests: 1 failed, 0 skipped, 12 passed")).toBe("error");
    expect(classify("✖ 3 problems (0 errors, 3 warnings)")).toBeNull();
  });
});

describe("compilePatterns", () => {
  it("adds per-source patterns, case-insensitive by default", () => {
    const p = compilePatterns({
      errorPatterns: ["boom"],
      warnPatterns: ["/Slow query/"],
      ignorePatterns: ["expected boom"],
    });
    expect(classify("BOOM happened", p)).toBe("error");
    expect(classify("Slow query detected", p)).toBe("warn");
    expect(classify("slow query detected", p)).toBeNull();
    expect(classify("expected boom in tests", p)).toBeNull();
  });

  it("drops invalid patterns instead of throwing", () => {
    expect(parsePattern("(unclosed")).toBeNull();
    const p = compilePatterns({ errorPatterns: ["(unclosed", "ok"] });
    expect(classify("ok then", p)).toBe("error");
  });

  it("keeps explicit flags", () => {
    expect(parsePattern("/abc/")?.flags).toBe("");
    expect(parsePattern("/abc/gi")?.flags).toBe("gi");
    expect(parsePattern("abc")?.flags).toBe("i");
  });
});

describe("MarkerCollector", () => {
  it("collapses consecutive identical lines and skips re-scans", () => {
    const c = new MarkerCollector();
    expect(c.feed(10, "Error: x", "error", 1)?.count).toBe(1);
    expect(c.feed(11, "Error: x", "error", 2)).toBeNull();
    expect(c.feed(11, "Error: x", "error", 3)).toBeNull();
    expect(c.markers).toHaveLength(1);
    expect(c.markers[0]?.count).toBe(2);
    expect(c.markers[0]?.index).toBe(11);
    expect(c.feed(13, "Error: x", "error", 4)?.count).toBe(1);
    expect(c.markers).toHaveLength(2);
  });

  it("ignores unclassified lines and finds neighbours", () => {
    const c = new MarkerCollector();
    c.feed(1, "ok", null, 0);
    c.feed(2, "warn: a", "warn", 0);
    c.feed(5, "Error: b", "error", 0);
    c.feed(9, "Error: c", "error", 0);
    expect(c.latest("error")?.index).toBe(9);
    expect(c.latest("warn")?.index).toBe(2);
    expect(c.after(null)?.index).toBe(2);
    expect(c.after(2)?.index).toBe(5);
    expect(c.after(9)).toBeUndefined();
    expect(c.before(null)?.index).toBe(9);
    expect(c.before(5)?.index).toBe(2);
    expect(c.before(2)).toBeUndefined();
  });

  it("caps its memory", () => {
    const c = new MarkerCollector(3);
    for (let i = 0; i < 10; i++) c.feed(i * 2, `Error ${i}`, "error", 0);
    expect(c.markers.map((m) => m.text)).toEqual(["Error 7", "Error 8", "Error 9"]);
  });
});
