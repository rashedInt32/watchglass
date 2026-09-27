// Scripted tmux panes, Claude sessions, and verdicts for running the UI in a
// plain browser. Output is written as a terminal would deliver it: CRLF line
// endings and ANSI colour.
import type { Appearance } from "./appearance";
import type { Backend } from "./backend";
import type { ChunkMsg, ClaudeSession, Level, PaneInfo, Subscriber, Summary, VerdictMsg } from "./ipc";
import { rank } from "./verdict";

const MOCK_APPEARANCE: Appearance = {
  fontFamily: ["JetBrains Mono Nerd Font", "JetBrains Mono"],
  fontSize: 14,
  themeName: "night-owl",
  theme: {
    background: "#011627",
    foreground: "#d6deeb",
    cursor: "#7e57c2",
    selectionBackground: "#5f7e97",
    selectionForeground: "#dfe5ee",
    palette: [
      "#011627", "#ef5350", "#22da6e", "#addb67", "#82aaff", "#c792ea", "#21c7a8", "#ffffff",
      "#575656", "#ef5350", "#22da6e", "#ffeb95", "#82aaff", "#c792ea", "#7fdbca", "#ffffff",
    ],
  },
  source: "mock",
};

const pane = (id: string, session: string, windowIndex: number, windowName: string, command: string, title: string, cols = 120, rows = 34, active = false): PaneInfo => ({
  id, session, windowIndex, windowName, paneIndex: 0, pid: 1000 + Number(id.slice(1)), command, title, cols, rows, cwd: `/Users/you/code/${session}`, active, attached: true, piped: false,
});

const PANES: PaneInfo[] = [
  pane("%1", "shop", 1, "web", "node", "next dev", 140, 40, true),
  pane("%2", "shop", 2, "tests", "node", "vitest", 120, 34),
  pane("%3", "shop", 3, "types", "node", "tsc --watch", 120, 34),
  pane("%4", "infra", 1, "db", "docker", "docker compose up", 160, 45),
  pane("%5", "main", 1, "claude", "claude", "✳ Refactor checkout form", 180, 50),
  pane("%6", "api", 1, "claude", "claude", "✳ Fix flaky payment test", 180, 50),
  pane("%7", "nvim", 1, "zsh", "zsh", "192.168.1.6", 230, 60),
  pane("%8", "packages", 1, "zsh", "zsh", "192.168.1.6", 230, 60),
];

const SESSIONS: ClaudeSession[] = [
  {
    sessionId: "aaaa-1", pid: 1005, name: "claude-62", cwd: "/Users/you/code/shop", status: "idle",
    statusUpdatedAt: Date.now() - 40_000, updatedAt: Date.now() - 40_000, tmux: "main:@0.%5", paneId: "%5",
    lastText: "Done. The checkout form now validates on blur. Want me to keep the two new tests, or drop them?",
    lastTextAt: new Date().toISOString(),
  },
  {
    sessionId: "bbbb-2", pid: 1006, name: "claude-63", cwd: "/Users/you/code/api", status: "busy",
    statusUpdatedAt: Date.now() - 5_000, updatedAt: Date.now() - 5_000, tmux: "api:@0.%6", paneId: "%6",
    lastText: "Re-running the payment suite with the retry removed.",
    lastTextAt: new Date().toISOString(),
  },
  {
    sessionId: "cccc-3", pid: 1007, name: "claude-64", cwd: "/Users/you/code/docs", status: "waiting",
    statusUpdatedAt: Date.now() - 90_000, updatedAt: Date.now() - 90_000, tmux: "nvim:@0.%7", paneId: "%7",
    lastText: "I need permission to run `pnpm publish --dry-run`.",
    lastTextAt: new Date().toISOString(),
  },
];

type Step = { wait: number; text?: string; exit?: number };
type Script = () => Generator<Step, void, unknown>;

const esc = (code: string, s: string) => `\x1b[${code}m${s}\x1b[0m`;
const dim = (s: string) => esc("2", s);
const bold = (s: string) => esc("1", s);
const red = (s: string) => esc("31", s);
const green = (s: string) => esc("32", s);
const yellow = (s: string) => esc("33", s);
const blue = (s: string) => esc("34", s);
const magenta = (s: string) => esc("35", s);
const cyan = (s: string) => esc("36", s);
const gray = (s: string) => esc("90", s);
const nl = (...lines: string[]) => lines.map((l) => l + "\r\n").join("");
const pick = <T,>(xs: T[]): T => xs[Math.floor(Math.random() * xs.length)]!;
const between = (a: number, b: number) => a + Math.random() * (b - a);
const clock = () => new Date().toLocaleTimeString("en-US", { hour12: true });

const web: Script = function* () {
  yield { wait: 300, text: nl(`   ${bold("▲ Next.js 15.5.2")}`, `   - Local:        http://localhost:3000`, "") };
  yield { wait: 800, text: nl(` ${green("✓")} Ready in 812ms`) };
  const routes = ["/", "/dashboard", "/api/session", "/settings", "/api/users?page=2", "/login"];
  let n = 0;
  while (true) {
    n++;
    const route = pick(routes);
    if (n % 5 === 1) {
      yield { wait: between(500, 1200), text: nl(` ${gray("○")} Compiling ${route} ...`) };
      yield { wait: between(300, 900), text: nl(` ${green("✓")} Compiled ${route} in ${Math.round(between(200, 1400))}ms (${Math.round(between(300, 900))} modules)`) };
    }
    if (n % 11 === 0) {
      yield { wait: 600, text: nl(` ${yellow("⚠")} Fast Refresh had to perform a full reload due to a runtime error.`) };
      yield { wait: 200, text: nl(` ${red("⨯")} ${red("TypeError: Cannot read properties of undefined (reading 'map')")}`, `    at DashboardPage ${dim("(app/dashboard/page.tsx:42:18)")}`, ` GET /dashboard ${red("500")} in 91ms`) };
      continue;
    }
    yield { wait: between(700, 2600), text: nl(` GET ${route} ${green("200")} in ${Math.round(between(8, 640))}ms`) };
  }
};

const tests: Script = function* () {
  yield { wait: 500, text: nl(`${esc("1;44", " RUN ")} ${bold("v3.2.4")} ${gray("/Users/you/code/shop")}`, "") };
  let failing = true;
  while (true) {
    yield { wait: 600, text: nl(` ${green("✓")} src/lib/detect.test.ts ${dim("(24 tests)")} ${dim("14ms")}`) };
    if (failing) {
      yield { wait: 500, text: nl(` ${yellow("❯")} src/components/Table.test.tsx ${dim("(5 tests | 1 failed)")}`, `   ${red("×")} renders one row per record`, `     ${red("→ expected 3 to be 4")}`, "", ` ${dim("Test Files")}  ${red("1 failed")} ${dim("|")} ${green("2 passed")}`, `${esc("1;41", " FAIL ")} ${red("Tests failed. Watching for file changes...")}`, "") };
    } else {
      yield { wait: 500, text: nl(` ${green("✓")} src/components/Table.test.tsx ${dim("(5 tests)")}`, "", ` ${dim("Test Files")}  ${green("3 passed")} ${dim("(3)")}`, `${esc("1;42", " PASS ")} ${green("Waiting for file changes...")}`, "") };
    }
    failing = !failing;
    yield { wait: between(9000, 16000), text: nl(`${esc("1;44", " RERUN ")} src/components/Table.test.tsx`, "") };
  }
};

const types: Script = function* () {
  yield { wait: 400, text: nl(`${gray(`[${clock()}]`)} Starting compilation in watch mode...`, "") };
  let broken = true;
  while (true) {
    if (broken) {
      yield { wait: 1800, text: nl(`${cyan("src/lib/table.ts")}:${yellow("18")}:${yellow("7")} - ${red("error")} ${gray("TS2322")}: Type 'string' is not assignable to type 'number'.`, "", `${gray(`[${clock()}]`)} Found 1 error. Watching for file changes.`, "") };
    } else {
      yield { wait: 1200, text: nl(`${gray(`[${clock()}]`)} Found 0 errors. Watching for file changes.`, "") };
    }
    broken = !broken;
    yield { wait: between(12000, 20000), text: nl(`${gray(`[${clock()}]`)} File change detected. Starting incremental compilation...`, "") };
  }
};

const db: Script = function* () {
  const svc = (name: string, color: (s: string) => string, msg: string) => `${color(name.padEnd(8))} ${gray("|")} ${msg}`;
  yield { wait: 300, text: nl(svc("db-1", cyan, `${clock()} UTC [1] LOG:  database system is ready to accept connections`)) };
  yield { wait: 300, text: nl(svc("redis-1", magenta, `1:M ${clock()} * Ready to accept connections tcp`)) };
  let n = 0;
  while (true) {
    n++;
    if (n % 9 === 0) {
      yield { wait: 800, text: nl(svc("api-1", green, red("Error: connect ECONNREFUSED 127.0.0.1:6379")), svc("api-1", green, yellow(`{"level":"warn","msg":"retrying redis in 2s"}`))) };
      continue;
    }
    const path = pick(["/v1/users", "/v1/orders/8812", "/v1/health", "/v1/search?q=lamp"]);
    yield { wait: between(500, 2200), text: nl(svc("api-1", green, `${pick(["GET", "POST"])} ${path} ${blue("200")} ${Math.round(between(3, 120))}ms`)) };
  }
};

const claudeIdle: Script = function* () {
  yield { wait: 200, text: nl(`${gray("╭─")} ${bold("Claude Code")} ${gray("v2.1")}`, "", `${cyan("●")} Done. The checkout form now validates on blur.`, "", `  Want me to keep the two new tests, or drop them?`, "", `${gray("╰─")} ${dim("? for shortcuts")}`) };
  while (true) yield { wait: 60_000 };
};

const claudeBusy: Script = function* () {
  yield { wait: 200, text: nl(`${gray("╭─")} ${bold("Claude Code")} ${gray("v2.1")}`, "", `${cyan("●")} Re-running the payment suite with the retry removed.`, "") };
  const spin = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴"];
  let i = 0;
  while (true) {
    i++;
    yield { wait: 120, text: `\r${magenta(spin[i % spin.length]!)} ${dim(`Running… (${i}s · 12k tokens)`)}` };
  }
};

const shell: Script = function* () {
  yield { wait: 100, text: nl(`${green("➜")}  ${cyan("~/code")} ${dim("git:(main)")} `) };
  while (true) yield { wait: 60_000 };
};

const SCRIPTS: Record<string, Script> = { "%1": web, "%2": tests, "%3": types, "%4": db, "%5": claudeIdle, "%6": claudeBusy, "%7": shell, "%8": shell };

function runScript(script: Script, emit: (text: string) => void): () => void {
  let stopped = false;
  let timer = 0;
  const it = script();
  const tick = () => {
    if (stopped) return;
    const { value, done } = it.next();
    if (done || !value) return;
    if (value.text) emit(value.text);
    timer = window.setTimeout(tick, value.wait);
  };
  tick();
  return () => {
    stopped = true;
    clearTimeout(timer);
  };
}

function toBase64(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

const VERDICT_CYCLE: Record<string, Level[]> = {
  "%1": ["working", "working", "failing", "working"],
  "%2": ["failing", "working"],
  "%3": ["failing", "working"],
  "%4": ["working", "warning", "working"],
  "%5": ["attention"],
  "%6": ["working"],
  "%7": ["idle"],
  "%8": ["idle"],
};

const SNIPPETS: Record<string, string> = {
  "%1": "GET /dashboard 500 in 91ms",
  "%2": "FAIL Tests failed. Watching for file changes...",
  "%3": "Found 1 error. Watching for file changes.",
  "%4": '{"level":"warn","msg":"retrying redis in 2s"}',
  "%7": "➜  ~/code git:(main)",
  "%8": "➜  ~/code git:(main)",
};

export function createMockBackend(): Backend {
  let sub: Subscriber | null = null;
  let summarySub: ((s: Summary) => void) | null = null;
  const stops = new Map<string, () => void>();
  const seqs = new Map<string, number>();
  const cycles = new Map<string, number>();
  const verdicts = new Map<string, VerdictMsg>();
  let verdictTimer = 0;

  const buildSummary = (): Summary => {
    const sessions = SESSIONS.map((s) => {
      const v = verdicts.get(s.sessionId);
      return {
        sessionId: s.sessionId, name: s.name, cwd: s.cwd, status: s.status, tmux: s.tmux, paneId: s.paneId,
        level: v?.level ?? "idle", confidence: v?.confidence ?? 0, source: v?.source ?? "none",
        snippet: s.lastText ?? "", updatedAt: s.updatedAt,
      } as const;
    }).sort((a, b) => rank(b.level) - rank(a.level) || b.updatedAt - a.updatedAt);
    const owned = new Map(SESSIONS.filter((s) => s.paneId).map((s) => [s.paneId!, s.sessionId]));
    const panes = PANES.map((p) => {
      const v = verdicts.get(owned.get(p.id) ?? p.id) ?? verdicts.get(p.id);
      return {
        id: p.id, session: p.session, windowIndex: p.windowIndex, windowName: p.windowName, paneIndex: p.paneIndex,
        command: p.command, title: p.title, cwd: p.cwd, level: v?.level ?? "idle", confidence: v?.confidence ?? 0,
        source: v?.source ?? "none", snippet: SNIPPETS[p.id] ?? p.title,
      } as const;
    }).sort((a, b) => rank(b.level) - rank(a.level) || a.session.localeCompare(b.session));
    const counts: Partial<Record<Level, number>> = {};
    let top: Level = "idle";
    for (const r of [...sessions, ...panes.filter((p) => !owned.has(p.id))]) {
      counts[r.level] = (counts[r.level] ?? 0) + 1;
      if (rank(r.level) > rank(top)) top = r.level;
    }
    return { updatedAt: Date.now(), top, counts, sessions, panes };
  };
  const pushVerdict = (v: VerdictMsg) => {
    verdicts.set(v.id, v);
    sub?.onVerdict(v);
  };

  const emit = (id: string, text: string) => {
    const seq = (seqs.get(id) ?? 0) + 1;
    seqs.set(id, seq);
    sub?.onChunk({ id, seq, t: Date.now(), data: toBase64(text) } satisfies ChunkMsg);
  };

  const verdict = (id: string, kind: "pane" | "claude", level: Level, source: "jev" | "rule" = "jev"): VerdictMsg => ({
    id, kind, level, confidence: 0.6 + Math.random() * 0.39, probabilities: { [level]: 0.8 }, source, at: Date.now(),
  });

  return {
    kind: "mock",
    async subscribe(s) {
      sub = s;
      setTimeout(() => {
        s.onPanes(PANES);
        s.onClaude(SESSIONS);
        pushVerdict(verdict("aaaa-1", "claude", "attention"));
        pushVerdict(verdict("bbbb-2", "claude", "working", "rule"));
        pushVerdict(verdict("cccc-3", "claude", "attention", "rule"));
        summarySub?.(buildSummary());
      }, 50);
      clearInterval(verdictTimer);
      verdictTimer = window.setInterval(() => {
        for (const [id, levels] of Object.entries(VERDICT_CYCLE)) {
          const n = (cycles.get(id) ?? 0) + 1;
          cycles.set(id, n);
          pushVerdict(verdict(id, "pane", levels[n % levels.length]!));
        }
        summarySub?.(buildSummary());
      }, 7000);
    },
    async subscribeSummary(onSummary) {
      summarySub = onSummary;
      if (!sub) {
        // The panel alone in a browser: seed the same story the board gets.
        pushVerdict(verdict("aaaa-1", "claude", "attention"));
        pushVerdict(verdict("bbbb-2", "claude", "working", "rule"));
        pushVerdict(verdict("cccc-3", "claude", "attention", "rule"));
        pushVerdict(verdict("%2", "pane", "failing"));
        pushVerdict(verdict("%4", "pane", "warning"));
        pushVerdict(verdict("%1", "pane", "working"));
        for (const id of ["%3", "%7", "%8"]) pushVerdict(verdict(id, "pane", "idle", "rule"));
      }
      onSummary(buildSummary());
    },
    async openMain() {
      console.info("[mock] open main window");
    },
    async hidePanel() {
      console.info("[mock] hide panel");
    },
    async verdictsPath() {
      return "~/.local/state/watchglass/verdicts.json";
    },
    async tmuxAvailable() {
      return true;
    },
    async tmuxStatus() {
      return { available: true, path: "/opt/homebrew/bin/tmux", socket: "/tmp/tmux-501/default", error: null };
    },
    async attachPane(id) {
      const p = PANES.find((x) => x.id === id);
      if (!p) throw new Error(`no pane ${id}`);
      stops.get(id)?.();
      stops.set(id, runScript(SCRIPTS[id] ?? web, (text) => emit(id, text)));
      return { id, cols: p.cols, rows: p.rows, snapshot: "" };
    },
    async detachPane(id) {
      const s = stops.get(id);
      if (!s) return false;
      s();
      stops.delete(id);
      return true;
    },
    async focusPane(id) {
      console.info("[mock] focus pane", id);
    },
    async sendKeys(id, keys) {
      console.info("[mock] send-keys", id, keys);
      // Answering a waiting session moves it on, as the real one would.
      const session = SESSIONS.find((s) => s.paneId === id && s.status === "waiting");
      if (!session) return;
      const approved = keys.includes("1");
      session.status = approved ? "busy" : "idle";
      session.lastText = approved ? "Running `pnpm publish --dry-run`…" : "Okay, skipping the publish.";
      session.updatedAt = Date.now();
      sub?.onClaude([...SESSIONS]);
      pushVerdict(verdict(session.sessionId, "claude", approved ? "working" : "idle", "rule"));
      summarySub?.(buildSummary());
    },
    async sendLine(id, text) {
      console.info("[mock] send-line", id, text);
      emit(id, nl(`${dim("›")} ${text}`));
    },
    async newWindow(session, name, command) {
      const id = `%${90 + PANES.length}`;
      const p = pane(id, session || "shop", 9, name || command.split(/\s+/)[0] || "new", command.split(/\s+/)[0] || "zsh", command, 120, 34);
      PANES.push(p);
      sub?.onPanes([...PANES]);
      console.info("[mock] new-window", session, name, command, "→", id);
      return id;
    },
    async jevStatus() {
      return { enabled: true, reason: "mock" };
    },
    async ghosttyAppearance() {
      return MOCK_APPEARANCE;
    },
  };
}
