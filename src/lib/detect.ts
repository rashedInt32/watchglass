// Line classification. Runs on rendered buffer lines, never on raw bytes,
// so carriage-return rewrites are judged on their final text (SPEC §5.3).

export type Severity = "error" | "warn";

export type Patterns = {
  error: RegExp[];
  warn: RegExp[];
  ignore: RegExp[];
};

export type Marker = {
  /** Buffer line index at detection time. May drift once scrollback is full. */
  index: number;
  text: string;
  severity: Severity;
  /** Detection time, ms since epoch. */
  t: number;
  /** Consecutive identical lines collapsed into this marker. */
  count: number;
};

export const DEFAULT_ERROR: RegExp[] = [
  /\berror\b/i,
  // TypeError, ReferenceError, HttpError… but not ErrorBoundary.
  /\b\w+Error\b/,
  /\bERR!/,
  /[✗✖×✕]/,
  /\bFAIL\b/,
  /\bfailed\b/i,
  /\bfailure\b/i,
  /\bException\b/,
  /\bTraceback\b/,
  /\bpanicked\b/,
  /\bUnhandled\b/,
  /\bAssertionError\b/,
  /\bE(?:ACCES|ADDRINUSE|AI_AGAIN|CONNREFUSED|CONNRESET|HOSTUNREACH|MFILE|NOENT|NOTFOUND|PERM|PIPE|TIMEDOUT)\b/,
];

export const DEFAULT_WARN: RegExp[] = [/\bwarn(?:ing)?\b/i, /\bdeprecated\b/i, /⚠/];

export const DEFAULT_IGNORE: RegExp[] = [
  // A zero-count summary ("0 errors", "0 failed") with no positive count on the same line.
  /^(?!.*\b[1-9]\d* (?:errors?|failed|failures?)\b).*\b0 (?:errors?|failed|failures?)\b/i,
  /\bFound 0 errors?\b/,
  /\bno errors?\b/i,
];

export const DEFAULT_PATTERNS: Patterns = {
  error: DEFAULT_ERROR,
  warn: DEFAULT_WARN,
  ignore: DEFAULT_IGNORE,
};

/**
 * `/body/flags` keeps its flags; anything else is case-insensitive.
 * Invalid patterns are dropped rather than breaking detection.
 */
export function parsePattern(p: string): RegExp | null {
  const m = /^\/(.+)\/([a-z]*)$/s.exec(p);
  try {
    return m ? new RegExp(m[1]!, m[2]) : new RegExp(p, "i");
  } catch {
    return null;
  }
}

export function compilePatterns(overrides: {
  errorPatterns?: string[];
  warnPatterns?: string[];
  ignorePatterns?: string[];
}): Patterns {
  const parse = (list: string[] | undefined) =>
    (list ?? []).map(parsePattern).filter((r): r is RegExp => r !== null);
  return {
    error: [...DEFAULT_ERROR, ...parse(overrides.errorPatterns)],
    warn: [...DEFAULT_WARN, ...parse(overrides.warnPatterns)],
    ignore: [...DEFAULT_IGNORE, ...parse(overrides.ignorePatterns)],
  };
}

export function classify(text: string, patterns: Patterns = DEFAULT_PATTERNS): Severity | null {
  if (text.trim() === "") return null;
  if (patterns.ignore.some((r) => r.test(text))) return null;
  if (patterns.error.some((r) => r.test(text))) return "error";
  if (patterns.warn.some((r) => r.test(text))) return "warn";
  return null;
}

/**
 * Feeds classified lines in buffer order and collapses consecutive
 * duplicates. Re-scanning an already-emitted line is a no-op, which
 * matters because the cursor line is scanned again on the next write.
 */
export class MarkerCollector {
  readonly markers: Marker[] = [];
  private emitted = new Map<number, string>();
  private readonly cap: number;

  constructor(cap = 5000) {
    this.cap = cap;
  }

  /** Returns the new marker, or null when the line was ignored or merged. */
  feed(index: number, text: string, severity: Severity | null, t: number): Marker | null {
    if (!severity) return null;
    if (this.emitted.get(index) === text) return null;
    this.emitted.set(index, text);
    const last = this.markers[this.markers.length - 1];
    if (last && last.text === text && last.severity === severity && index - last.index <= 1) {
      last.count += 1;
      last.index = index;
      return null;
    }
    const marker: Marker = { index, text, severity, t, count: 1 };
    this.markers.push(marker);
    if (this.markers.length > this.cap) {
      const dropped = this.markers.splice(0, this.markers.length - this.cap);
      for (const d of dropped) this.emitted.delete(d.index);
    }
    return marker;
  }

  latest(severity: Severity): Marker | undefined {
    for (let i = this.markers.length - 1; i >= 0; i--) {
      const m = this.markers[i]!;
      if (m.severity === severity) return m;
    }
    return undefined;
  }

  /** First marker strictly after `index`, or the earliest when `index` is null. */
  after(index: number | null): Marker | undefined {
    if (index === null) return this.markers[0];
    return this.markers.find((m) => m.index > index);
  }

  /** Last marker strictly before `index`, or the latest when `index` is null. */
  before(index: number | null): Marker | undefined {
    if (index === null) return this.markers[this.markers.length - 1];
    for (let i = this.markers.length - 1; i >= 0; i--) {
      const m = this.markers[i]!;
      if (m.index < index) return m;
    }
    return undefined;
  }
}
