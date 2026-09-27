import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { Terminal } from "ghostty-web";
import { backend } from "../lib/backend";
import { countLineFeeds, subscribeChunks } from "../lib/bus";
import { classify, DEFAULT_PATTERNS, MarkerCollector, type Marker } from "../lib/detect";
import { registerTerminal } from "../lib/devtools";
import { reduceFollow, type FollowEvent, type FollowMode } from "../lib/follow";
import { decodeChunk, type ClaudeSession, type PaneInfo, type VerdictMsg } from "../lib/ipc";
import {
  createMirrorTerminal,
  cursorIndex,
  fitMirror,
  lineText,
  locateLine,
  mirrorCanvas,
  scrollLineIntoView,
  setViewportY,
  viewportTop,
  type TerminalLook,
} from "../lib/terminal";
import { describe } from "../lib/verdict";

export type SearchHit = { id: string; index: number; text: string };

export type TileApi = {
  jumpTo(target: Pick<Marker, "index" | "text">): boolean;
  jumpToLive(): void;
  next(): boolean;
  prev(): boolean;
  toggleFollow(): void;
  search(query: string, limit?: number): SearchHit[];
  latestError(): Marker | undefined;
  /** Open the one-line command box. */
  openInput(): void;
};

type Props = {
  pane: PaneInfo;
  claude?: ClaudeSession;
  verdict?: VerdictMsg;
  look: TerminalLook;
  index: number;
  unread: number;
  active: boolean;
  focused: boolean;
  hiddenByFocus: boolean;
  onMarker: (id: string, marker: Marker) => void;
  onSelect: () => void;
  /** Move this pane to the hidden strip. Only the user does this. */
  onHide: () => void;
};

function GoIcon() {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
      <path d="M3 13 13 3M6 3h7v7" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export const PaneTile = forwardRef<TileApi, Props>(function PaneTile(props, ref) {
  const { pane, claude, verdict, look, index, unread, active, focused, hiddenByFocus, onMarker, onSelect, onHide } = props;
  const hostRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const collector = useRef(new MarkerCollector());
  const onMarkerRef = useRef(onMarker);
  onMarkerRef.current = onMarker;
  const lookRef = useRef(look);
  lookRef.current = look;
  const [follow, setFollow] = useState<FollowMode>("live");
  const followRef = useRef<FollowMode>("live");
  const lastJump = useRef<number | null>(null);
  const restoring = useRef(false);
  const [flash, setFlash] = useState<{ top: number; height: number; key: number } | null>(null);
  const [fontSize, setFontSize] = useState(look.fontSize);
  const [input, setInput] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const dispatch = useCallback((event: FollowEvent) => {
    const next = reduceFollow(followRef.current, event);
    if (next !== followRef.current) {
      followRef.current = next;
      setFollow(next);
      const canvas = termRef.current ? mirrorCanvas(termRef.current) : null;
      if (canvas && next === "paused") canvas.style.transform = "";
    }
  }, []);

  const detect = useCallback(
    (term: Terminal, lineFeeds: number, t: number) => {
      const buf = term.buffer.active;
      const cur = cursorIndex(term);
      const from = Math.max(0, cur - lineFeeds - 1);
      const to = Math.min(buf.length - 1, cur);
      for (let i = from; i <= to; i++) {
        const text = lineText(term, i);
        if (!text) continue;
        const marker = collector.current.feed(i, text, classify(text, DEFAULT_PATTERNS), t);
        if (marker) onMarkerRef.current(pane.id, marker);
      }
    },
    [pane.id],
  );

  /**
   * The mirror can be taller than the tile. Live: keep the cursor row (where
   * output lands) inside the clip. Paused or after a jump: show the top of
   * the viewport, where scrollLineIntoView places the target.
   */
  const keepCursorVisible = useCallback(() => {
    const term = termRef.current;
    const host = hostRef.current;
    const canvas = term ? mirrorCanvas(term) : null;
    if (!term || !host || !canvas) return;
    if (followRef.current === "paused") {
      canvas.style.transform = "";
      return;
    }
    // The canvas keeps its natural size; the tile clips it, so shift it up.
    const rowHeight = canvas.offsetHeight / term.rows;
    const needed = (term.buffer.active.cursorY + 1) * rowHeight + 2;
    const shift = Math.max(0, Math.ceil(needed - host.clientHeight));
    canvas.style.transform = shift > 0 ? `translateY(-${shift}px)` : "";
  }, []);

  const refit = useCallback(() => {
    const term = termRef.current;
    const box = boxRef.current;
    if (!term || !box) return;
    const r = box.getBoundingClientRect();
    if (r.width < 20 || r.height < 20) return;
    setFontSize(fitMirror(term, r.width, r.height, lookRef.current.fontFamily));
    keepCursorVisible();
  }, [keepCursorVisible]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let disposed = false;
    let term: Terminal | null = null;
    let unsubscribe: (() => void) | null = null;
    const disposables: { dispose(): void }[] = [];

    void (async () => {
      const created = await createMirrorTerminal(host, lookRef.current, pane.cols, pane.rows);
      if (disposed) {
        created.dispose();
        return;
      }
      term = created;
      termRef.current = term;
      registerTerminal(pane.id, term);
      refit();
      disposables.push(
        term.onScroll(() => {
          if (term && !restoring.current) dispatch({ type: "scroll", atBottom: term.getViewportY() < 0.5 });
        }),
      );
      const write = (bytes: Uint8Array, t_ms: number) => {
        if (!term) return;
        const lineFeeds = countLineFeeds(bytes);
        const t = term;
        const paused = followRef.current === "paused";
        const anchor = paused ? Math.round(t.getViewportY()) + lineFeeds : 0;
        if (paused) restoring.current = true;
        t.write(bytes, () => {
          if (paused) {
            setViewportY(t, anchor);
            restoring.current = false;
          }
          keepCursorVisible();
          detect(t, lineFeeds, t_ms);
        });
      };
      // Live chunks can arrive before the attach call returns the screen
      // snapshot; hold them so the snapshot always lands first.
      let queued: { bytes: Uint8Array; t: number }[] | null = [];
      unsubscribe = subscribeChunks(pane.id, (msg) => {
        const bytes = decodeChunk(msg.data);
        if (queued) queued.push({ bytes, t: msg.t });
        else write(bytes, msg.t);
      });
      try {
        const info = await backend.attachPane(pane.id);
        if (disposed) return;
        if (info.snapshot) write(decodeChunk(info.snapshot), Date.now());
      } catch (e) {
        term?.write(`\r\n\x1b[31m✗ ${String(e)}\x1b[0m\r\n`);
      }
      const held = queued;
      queued = null;
      for (const q of held) write(q.bytes, q.t);
    })();

    return () => {
      disposed = true;
      unsubscribe?.();
      for (const d of disposables) d.dispose();
      term?.dispose();
      registerTerminal(pane.id, null);
      termRef.current = null;
      void backend.detachPane(pane.id).catch(() => {});
    };
    // The terminal lives as long as the pane id does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pane.id]);

  // Follow the tmux pane's size; the font scales to keep it inside the tile.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    if (term.cols !== pane.cols || term.rows !== pane.rows) term.resize(pane.cols, pane.rows);
    refit();
  }, [pane.cols, pane.rows, refit]);

  useEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const ro = new ResizeObserver(() => refit());
    ro.observe(box);
    return () => ro.disconnect();
  }, [refit]);

  useEffect(() => {
    refit();
  }, [focused, hiddenByFocus, refit]);

  const flashLine = useCallback((term: Terminal, idx: number) => {
    const overlay = overlayRef.current;
    const canvas = mirrorCanvas(term);
    if (!overlay || !canvas) return;
    const row = idx - viewportTop(term);
    const rect = canvas.getBoundingClientRect();
    const rowHeight = rect.height / term.rows;
    // The canvas may be shifted up; the rect already includes that.
    const offset = rect.top - overlay.getBoundingClientRect().top;
    setFlash({ top: offset + row * rowHeight, height: rowHeight, key: Date.now() });
  }, []);

  const api = useMemo<TileApi>(() => {
    const self: TileApi = {
      jumpTo(target) {
        const term = termRef.current;
        if (!term) return false;
        const idx = locateLine(term, target.index, target.text);
        if (idx === null) return false;
        scrollLineIntoView(term, idx);
        dispatch({ type: "jump" });
        lastJump.current = idx;
        flashLine(term, idx);
        return true;
      },
      jumpToLive() {
        const term = termRef.current;
        if (!term) return;
        term.scrollToBottom();
        lastJump.current = null;
        dispatch({ type: "live" });
        keepCursorVisible();
      },
      next() {
        const m = lastJump.current === null ? undefined : collector.current.after(lastJump.current);
        return m ? self.jumpTo(m) : false;
      },
      prev() {
        const m = collector.current.before(lastJump.current);
        return m ? self.jumpTo(m) : false;
      },
      toggleFollow() {
        if (followRef.current === "paused") self.jumpToLive();
        else dispatch({ type: "toggle" });
      },
      search(query, limit = 100) {
        const term = termRef.current;
        const needle = query.trim().toLowerCase();
        if (!term || !needle) return [];
        const hits: SearchHit[] = [];
        const buf = term.buffer.active;
        for (let i = buf.length - 1; i >= 0 && hits.length < limit; i--) {
          const text = lineText(term, i);
          if (text && text.toLowerCase().includes(needle)) hits.push({ id: pane.id, index: i, text });
        }
        return hits;
      },
      latestError() {
        return collector.current.latest("error");
      },
      openInput() {
        setInput((v) => v ?? "");
        setTimeout(() => inputRef.current?.focus(), 0);
      },
    };
    return self;
  }, [dispatch, flashLine, keepCursorVisible, pane.id]);

  const sendInput = () => {
    const text = (input ?? "").trim();
    if (text) void backend.sendLine(pane.id, text);
    setInput(null);
  };

  useImperativeHandle(ref, () => api, [api]);

  const level = verdict?.level ?? "idle";
  const classes = ["tile", `tile--${level}`];
  if (active) classes.push("tile--active");
  if (focused) classes.push("tile--focused");
  if (hiddenByFocus) classes.push("tile--hidden");
  const label = `${pane.session}:${pane.windowName}${pane.paneIndex > 0 ? `.${pane.paneIndex}` : ""}`;

  return (
    <section className={classes.join(" ")} onMouseDown={onSelect} data-source={pane.id} data-level={level}>
      <header className="tile-head">
        <i className={`dot dot--${level}`} title={describe(verdict)} />
        <span className="tile-name">{label}</span>
        <span className="tile-index">{index + 1}</span>
        {claude ? (
          <span className="tile-claude" title={claude.cwd}>
            ✳ {claude.name}
          </span>
        ) : (
          <span className="tile-cmd" title={pane.title}>
            {pane.command}
            {pane.title && pane.title !== pane.command ? ` · ${pane.title}` : ""}
          </span>
        )}
        {unread > 0 && <span className="tile-badge">{unread}</span>}
        <span className={`tile-level tile-level--${level}`}>{describe(verdict)}</span>
        {follow === "paused" && (
          <button className="tile-live" onClick={() => api.jumpToLive()}>
            paused <span className="tile-live-arrow">↓</span> live
          </button>
        )}
        <span className="tile-size">
          {pane.cols}×{pane.rows} · {fontSize}px
        </span>
        <button className="tile-btn tile-btn--text" title="Type a line into this pane (i)" onClick={() => api.openInput()}>
          ›
        </button>
        <button className="tile-btn tile-btn--text" title="Send Enter" onClick={() => void backend.sendKeys(pane.id, ["Enter"])}>
          ⏎
        </button>
        <button className="tile-btn tile-btn--text" title="Send Ctrl-C" onClick={() => void backend.sendKeys(pane.id, ["C-c"])}>
          ^C
        </button>
        <button className="tile-btn" title="Go to this pane in tmux (g)" onClick={() => void backend.focusPane(pane.id)}>
          <GoIcon />
        </button>
        <button className="tile-btn tile-btn--text" title="Hide this pane (h); bring it back from the strip below" onClick={onHide}>
          –
        </button>
      </header>
      {input !== null && (
        <div className="tile-input" onMouseDown={(e) => e.stopPropagation()}>
          <span className="tile-input-prompt">›</span>
          <input
            ref={inputRef}
            className="tile-input-field"
            value={input}
            placeholder={`type into ${label}, Enter sends, Esc closes`}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") {
                e.preventDefault();
                sendInput();
              } else if (e.key === "Escape") {
                setInput(null);
              }
            }}
            spellCheck={false}
          />
        </div>
      )}
      <div className="tile-term" ref={boxRef}>
        <div className="tile-host" ref={hostRef} />
        <div className="tile-overlay" ref={overlayRef}>
          {flash && (
            <div
              key={flash.key}
              className="tile-flash"
              style={{ top: flash.top, height: flash.height }}
              onAnimationEnd={() => setFlash(null)}
            />
          )}
        </div>
      </div>
    </section>
  );
});
