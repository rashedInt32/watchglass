import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import "./App.css";
import { ClaudePane } from "./components/ClaudePane";
import { DemoCaption } from "./components/DemoCaption";
import { EmptyState } from "./components/EmptyState";
import { IdleStrip } from "./components/IdleStrip";
import { KeysHelp } from "./components/KeysHelp";
import { NewPane } from "./components/NewPane";
import { Palette } from "./components/Palette";
import { PaneTile, type SearchHit, type TileApi } from "./components/PaneTile";
import { TopBar } from "./components/TopBar";
import { chromeVars, defaultFontSize, fontFamilyCss, toTerminalTheme, type Appearance } from "./lib/appearance";
import { backend } from "./lib/backend";
import { publish } from "./lib/bus";
import type { Marker } from "./lib/detect";
import { SHELLS, type ClaudeSession, type JevStatus, type PaneInfo, type TmuxStatus, type VerdictMsg } from "./lib/ipc";
import { notify } from "./lib/notify";
import type { TerminalLook } from "./lib/terminal";
import { comparePriority, LEVEL_WORD, shouldNotify, tmuxOrder } from "./lib/verdict";

const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
/** A newly seen pane stays a tile this long before it may collapse as an idle shell. */
const NEW_PANE_GRACE_MS = 60_000;

export default function App() {
  const [tmuxOk, setTmuxOk] = useState<TmuxStatus | null>(null);
  const [panes, setPanes] = useState<PaneInfo[]>([]);
  const [sessions, setSessions] = useState<ClaudeSession[]>([]);
  const [verdicts, setVerdicts] = useState<Record<string, VerdictMsg>>({});
  const [jev, setJev] = useState<JevStatus | null>(null);
  const [appearance, setAppearance] = useState<Appearance | null | undefined>(undefined);
  const [unread, setUnread] = useState<Record<string, number>>({});
  const [activeId, setActiveId] = useState<string | null>(null);
  const [focusMode, setFocusMode] = useState(false);
  const [columns, setColumns] = useState(() => Number(localStorage.getItem("wg.columns") ?? "2") || 2);
  const [sortByPriority, setSortByPriority] = useState(() => localStorage.getItem("wg.sort") !== "tmux");
  const [palette, setPalette] = useState(false);
  const [help, setHelp] = useState(false);
  const [newPane, setNewPane] = useState(false);
  /** Idle shells the user asked to see anyway. */
  const [pinned, setPinned] = useState<Set<string>>(new Set());
  /** When each pane was first seen; new panes stay tiles for a while. */
  const firstSeen = useRef(new Map<string, number>());
  const [, bumpClock] = useState(0);

  const tiles = useRef(new Map<string, TileApi>());
  const latestError = useRef<{ id: string; marker: Marker } | null>(null);
  const verdictsRef = useRef(verdicts);
  verdictsRef.current = verdicts;
  const activeRef = useRef(activeId);
  activeRef.current = activeId;
  const focusRef = useRef(focusMode);
  focusRef.current = focusMode;
  const panesRef = useRef(panes);
  panesRef.current = panes;
  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;

  const theme = useMemo(() => toTerminalTheme(appearance ?? null), [appearance]);
  const mono = useMemo(() => fontFamilyCss(appearance ?? null), [appearance]);
  const look = useMemo<TerminalLook>(
    () => ({ theme, fontFamily: mono, fontSize: defaultFontSize(appearance ?? null) }),
    [theme, mono, appearance],
  );

  /** A Claude session's own verdict outranks the generic pane verdict for its pane. */
  const paneVerdict = useCallback(
    (paneId: string): VerdictMsg | undefined => {
      const session = sessionsRef.current.find((s) => s.paneId === paneId);
      const own = session ? verdictsRef.current[session.sessionId] : undefined;
      return own ?? verdictsRef.current[paneId];
    },
    [],
  );

  const onVerdict = useCallback((v: VerdictMsg) => {
    const prev = verdictsRef.current[v.id]?.level;
    setVerdicts((old) => ({ ...old, [v.id]: v }));
    if (!shouldNotify(prev, v.level)) return;
    const paneId = v.kind === "pane" ? v.id : sessionsRef.current.find((s) => s.sessionId === v.id)?.paneId ?? null;
    const isActive = paneId !== null && paneId === activeRef.current;
    const hidden = focusRef.current && !isActive;
    if (isActive && !hidden && document.hasFocus()) return;
    const pane = panesRef.current.find((p) => p.id === paneId);
    const session = v.kind === "claude" ? sessionsRef.current.find((s) => s.sessionId === v.id) : undefined;
    const title = session
      ? `Claude · ${session.cwd.split("/").pop()} · ${LEVEL_WORD[v.level]}`
      : pane
        ? `${pane.session}:${pane.windowName} · ${LEVEL_WORD[v.level]}`
        : `watchglass · ${LEVEL_WORD[v.level]}`;
    const body = session?.lastText ?? pane?.title ?? "";
    void notify(v.id, title, body);
  }, []);

  useEffect(() => {
    void (async () => {
      setTmuxOk(
        await backend
          .tmuxStatus()
          .catch((e) => ({ available: false, path: "?", socket: "?", error: String(e) })),
      );
      setJev(await backend.jevStatus().catch(() => ({ enabled: false, reason: "unavailable" })));
      const a = await backend.ghosttyAppearance().catch(() => null);
      setAppearance(a);
      await backend
        .subscribe({
          onChunk: publish,
          onPanes: (list) => setPanes(list),
          onClaude: (list) => setSessions(list),
          onVerdict,
        })
        .catch((e) => setTmuxOk({ available: false, path: "?", socket: "?", error: String(e) }));
    })();
  }, [onVerdict]);

  // Remember when each pane appeared, and re-evaluate the grid once the
  // newest one is old enough to collapse.
  useEffect(() => {
    const now = Date.now();
    const seen = firstSeen.current;
    for (const p of panes) if (!seen.has(p.id)) seen.set(p.id, now);
    for (const id of [...seen.keys()]) if (!panes.some((p) => p.id === id)) seen.delete(id);
    const youngest = Math.max(0, ...panes.map((p) => seen.get(p.id) ?? 0));
    const wait = NEW_PANE_GRACE_MS - (now - youngest) + 50;
    if (wait > 0 && wait < NEW_PANE_GRACE_MS + 100) {
      const t = setTimeout(() => bumpClock((n) => n + 1), wait);
      return () => clearTimeout(t);
    }
    return undefined;
  }, [panes]);

  /**
   * A shell sitting at its prompt with nothing to say, and no Claude in it.
   * A pane that just appeared is shown as a tile for a while first, so a
   * new session is seen arriving instead of vanishing into the strip.
   */
  const isIdleShell = useCallback(
    (p: PaneInfo): boolean => {
      if (pinned.has(p.id) || !SHELLS.has(p.command)) return false;
      if (sessionsRef.current.some((s) => s.paneId === p.id)) return false;
      if (Date.now() - (firstSeen.current.get(p.id) ?? 0) < NEW_PANE_GRACE_MS) return false;
      const level = paneVerdict(p.id)?.level;
      return level === undefined || level === "idle";
    },
    [paneVerdict, pinned],
  );

  const { ordered, collapsed } = useMemo(() => {
    const list = [...panes].sort(tmuxOrder);
    const shown = list.filter((p) => !isIdleShell(p));
    const hidden = list.filter((p) => isIdleShell(p));
    if (sortByPriority) shown.sort((a, b) => comparePriority(paneVerdict(a.id)?.level, paneVerdict(b.id)?.level));
    return { ordered: shown, collapsed: hidden };
    // verdicts is a dependency through paneVerdict's refs; re-sort when it changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [panes, sortByPriority, verdicts, sessions, isIdleShell]);

  const select = useCallback((id: string | null, focus?: boolean) => {
    setActiveId(id);
    if (focus !== undefined) setFocusMode(focus);
    if (id) setUnread((prev) => (prev[id] ? { ...prev, [id]: 0 } : prev));
  }, []);

  useEffect(() => {
    if (ordered.length === 0) return;
    if (!activeId || !ordered.some((p) => p.id === activeId)) setActiveId(ordered[0]!.id);
  }, [ordered, activeId]);

  const onMarker = useCallback((id: string, marker: Marker) => {
    if (marker.severity !== "error") return;
    latestError.current = { id, marker };
    const isActive = id === activeRef.current;
    const hidden = focusRef.current && !isActive;
    if (!isActive || hidden || !document.hasFocus()) {
      setUnread((prev) => ({ ...prev, [id]: (prev[id] ?? 0) + 1 }));
    }
  }, []);

  const jumpLatest = useCallback(() => {
    const latest = latestError.current;
    if (!latest) return;
    select(latest.id);
    tiles.current.get(latest.id)?.jumpTo(latest.marker);
  }, [select]);

  const jumpToHit = useCallback(
    (hit: SearchHit) => {
      select(hit.id);
      tiles.current.get(hit.id)?.jumpTo(hit);
      setPalette(false);
    },
    [select],
  );

  const orderedRef = useRef(ordered);
  orderedRef.current = ordered;

  useEffect(() => {
    function activeTile(): TileApi | undefined {
      return activeRef.current ? tiles.current.get(activeRef.current) : undefined;
    }
    function onKey(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      const typing = !!target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA");
      const mod = e.metaKey || e.ctrlKey;
      if (mod) {
        switch (e.key) {
          case "k":
            e.preventDefault();
            setPalette((p) => !p);
            return;
          case "j":
            e.preventDefault();
            jumpLatest();
            return;
        }
        if (/^[1-9]$/.test(e.key)) {
          e.preventDefault();
          const pane = orderedRef.current[Number(e.key) - 1];
          if (pane) select(pane.id, true);
        }
        return;
      }
      if (typing) return;
      switch (e.key) {
        case "Escape":
          setFocusMode(false);
          setPalette(false);
          setHelp(false);
          setNewPane(false);
          break;
        case "Enter":
          setFocusMode((f) => !f);
          break;
        case "?":
          setHelp((h) => !h);
          break;
        case "i":
          e.preventDefault();
          activeTile()?.openInput();
          break;
        case "N":
          e.preventDefault();
          setNewPane(true);
          break;
        case "f":
          activeTile()?.toggleFollow();
          break;
        case "n":
          activeTile()?.next();
          break;
        case "p":
          activeTile()?.prev();
          break;
        case "g":
          if (activeRef.current) void backend.focusPane(activeRef.current);
          break;
        case "s":
          setSortByPriority((on) => {
            localStorage.setItem("wg.sort", on ? "tmux" : "priority");
            return !on;
          });
          break;
        case "j":
        case "k": {
          const list = orderedRef.current;
          if (list.length === 0) break;
          const i = Math.max(0, list.findIndex((p) => p.id === activeRef.current));
          const delta = e.key === "j" ? 1 : -1;
          select(list[(i + delta + list.length) % list.length]!.id);
          break;
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [jumpLatest, select]);

  function setColumnsPersist(n: number) {
    setColumns(n);
    localStorage.setItem("wg.columns", String(n));
  }

  function setSortPersist(on: boolean) {
    setSortByPriority(on);
    localStorage.setItem("wg.sort", on ? "priority" : "tmux");
  }

  const counts = useMemo(() => {
    let attention = 0;
    let failing = 0;
    for (const p of panes) {
      const level = paneVerdict(p.id)?.level;
      if (level === "attention") attention++;
      if (level === "failing") failing++;
    }
    for (const s of sessions) {
      if (s.paneId) continue; // counted through its pane
      const level = verdicts[s.sessionId]?.level;
      if (level === "attention") attention++;
      if (level === "failing") failing++;
    }
    return { attention, failing };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [panes, sessions, verdicts]);

  const rootStyle = { ...chromeVars(theme, mono), "--cols": columns } as CSSProperties;
  const ready = appearance !== undefined;
  const sessionByPane = useMemo(() => new Map(sessions.filter((s) => s.paneId).map((s) => [s.paneId!, s])), [sessions]);

  return (
    <div className="app" style={rootStyle}>
      <TopBar
        panes={panes.length}
        idle={collapsed.length}
        attention={counts.attention}
        failing={counts.failing}
        sortByPriority={sortByPriority}
        onSort={setSortPersist}
        columns={columns}
        onColumns={setColumnsPersist}
        jev={jev}
        overlay={backend.kind === "tauri" && isMac}
        help={help}
        onToggleHelp={() => setHelp((h) => !h)}
        onNewPane={() => setNewPane(true)}
      />
      <div className="body">
        <ClaudePane
          sessions={sessions}
          verdicts={verdicts}
          activePaneId={activeId}
          onSelect={(s) => {
            if (s.paneId) select(s.paneId);
          }}
          onGo={(s) => {
            if (s.paneId) void backend.focusPane(s.paneId);
          }}
          onAnswer={(s, answer) => {
            if (s.paneId) void backend.sendKeys(s.paneId, [answer === "approve" ? "1" : "Escape"]);
          }}
        />
        <div className="main">
        {ready && ordered.length === 0 && <EmptyState tmux={tmuxOk} panes={panes.length} />}
        {ready && ordered.length > 0 && (
          <main className={focusMode ? "grid grid--focus" : "grid"}>
            {ordered.map((p, i) => (
              <PaneTile
                key={p.id}
                ref={(api) => {
                  if (api) tiles.current.set(p.id, api);
                  else tiles.current.delete(p.id);
                }}
                pane={p}
                claude={sessionByPane.get(p.id)}
                verdict={paneVerdict(p.id)}
                look={look}
                index={i}
                unread={unread[p.id] ?? 0}
                active={p.id === activeId}
                focused={focusMode && p.id === activeId}
                hiddenByFocus={focusMode && p.id !== activeId}
                onMarker={onMarker}
                onSelect={() => select(p.id)}
              />
            ))}
          </main>
        )}
        {ready && !focusMode && (
          <IdleStrip panes={collapsed} onExpand={(id) => setPinned((s) => new Set(s).add(id))} />
        )}
        </div>
      </div>
      {palette && (
        <Palette tiles={tiles.current} order={ordered.map((p) => p.id)} onJump={jumpToHit} onClose={() => setPalette(false)} />
      )}
      {help && <KeysHelp onClose={() => setHelp(false)} />}
      {backend.kind === "mock" && <DemoCaption />}
      {newPane && (
        <NewPane
          sessions={[...new Set(panes.map((p) => p.session))]}
          onCreate={async (session, name, command) => {
            const id = await backend.newWindow(session, name, command);
            setPinned((s) => new Set(s).add(id));
            select(id);
          }}
          onClose={() => setNewPane(false)}
        />
      )}
    </div>
  );
}
