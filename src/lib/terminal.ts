// ghostty-web setup shared by every tile. Tiles mirror a tmux pane at its
// real column and row count and scale the font to fit the tile.
import { Ghostty, Terminal } from "ghostty-web";
import wasmUrl from "ghostty-web/ghostty-vt.wasm?url";
import type { ResolvedTheme } from "./appearance";

export const SCROLLBACK = 10_000;
export const MIN_FONT = 7;
export const MAX_FONT = 22;

export type TerminalLook = {
  theme: ResolvedTheme;
  fontFamily: string;
  fontSize: number;
};

let engine: Promise<Ghostty> | null = null;

/** Loads the VT engine once per page. */
export function loadEngine(): Promise<Ghostty> {
  engine ??= Ghostty.load(wasmUrl);
  return engine;
}

export async function createMirrorTerminal(
  parent: HTMLElement,
  look: TerminalLook,
  cols: number,
  rows: number,
): Promise<Terminal> {
  const ghostty = await loadEngine();
  const term = new Terminal({
    ghostty,
    cols,
    rows,
    disableStdin: true,
    cursorBlink: false,
    convertEol: false,
    scrollback: SCROLLBACK,
    smoothScrollDuration: 0,
    fontSize: look.fontSize,
    fontFamily: look.fontFamily,
    theme: look.theme,
  });
  term.open(parent);
  if (term.textarea) {
    term.textarea.tabIndex = -1;
    term.textarea.blur();
  }
  return term;
}

const ratioCache = new Map<string, number>();

/** Glyph advance per pixel of font size for a monospace family. */
export function charRatio(fontFamily: string): number {
  const cached = ratioCache.get(fontFamily);
  if (cached) return cached;
  let ratio = 0.6;
  try {
    const ctx = document.createElement("canvas").getContext("2d");
    if (ctx) {
      ctx.font = `100px ${fontFamily}`;
      ratio = ctx.measureText("MMMMMMMMMM").width / 10 / 100;
    }
  } catch {
    // canvas unavailable: keep the monospace default
  }
  if (!(ratio > 0.3 && ratio < 1)) ratio = 0.6;
  ratioCache.set(fontFamily, ratio);
  return ratio;
}

/**
 * Largest font size at which `cols` fit the box width. Height is allowed to
 * overflow: the tile shows the bottom of a tall pane rather than shrinking
 * the text below legibility. Focus mode gives the room for all rows.
 */
export function fitFontSize(boxW: number, boxH: number, cols: number, rows: number, fontFamily: string): number {
  const byWidth = boxW / (Math.max(1, cols) * charRatio(fontFamily));
  const byHeight = boxH / (Math.max(1, rows) * 1.25);
  const size = Math.floor(byHeight >= MIN_FONT ? Math.min(byWidth, byHeight) : byWidth);
  return Math.max(MIN_FONT, Math.min(MAX_FONT, size));
}

/** Applies a font size in place and confirms the engine took it. */
export function applyFontSize(term: Terminal, size: number): boolean {
  try {
    (term.options as { fontSize: number }).fontSize = size;
    return term.options.fontSize === size;
  } catch {
    return false;
  }
}

/** ghostty-web draws into one canvas appended to the element it was opened in. */
export function mirrorCanvas(term: Terminal): HTMLCanvasElement | null {
  return term.element?.querySelector("canvas") ?? null;
}

/**
 * Fits the terminal's width inside the box, shrinking a step at a time when
 * the engine's real cell metrics come out wider than the estimate.
 */
export function fitMirror(term: Terminal, boxW: number, boxH: number, fontFamily: string): number {
  let size = fitFontSize(boxW, boxH, term.cols, term.rows, fontFamily);
  for (let i = 0; i < 6; i++) {
    applyFontSize(term, size);
    const canvas = mirrorCanvas(term);
    if (!canvas) break;
    if (canvas.getBoundingClientRect().width <= boxW + 1 || size <= MIN_FONT) break;
    size -= 1;
  }
  return size;
}

/** Absolute buffer index of the cursor line (0 = top of scrollback). */
export function cursorIndex(term: Terminal): number {
  const buf = term.buffer.active;
  return Math.max(0, buf.length - term.rows) + buf.cursorY;
}

/** Buffer index of the first visible row. */
export function viewportTop(term: Terminal): number {
  const buf = term.buffer.active;
  return Math.max(0, buf.length - term.rows - Math.round(term.getViewportY()));
}

export function lineText(term: Terminal, index: number): string {
  return term.buffer.active.getLine(index)?.translateToString(true) ?? "";
}

/**
 * Finds the buffer index whose text matches `text`, starting at the remembered
 * index and widening outward. Lines drift when the scrollback is full.
 */
export function locateLine(term: Terminal, index: number, text: string, radius = 4000): number | null {
  const length = term.buffer.active.length;
  if (index >= 0 && index < length && lineText(term, index) === text) return index;
  for (let d = 1; d <= radius; d++) {
    const up = index - d;
    const down = index + d;
    if (up < 0 && down >= length) break;
    if (up >= 0 && lineText(term, up) === text) return up;
    if (down < length && lineText(term, down) === text) return down;
  }
  return null;
}

/**
 * Viewport offset from the bottom, in lines. ghostty-web 0.4 ignores
 * `scrollToLine`, so all positioning goes through relative `scrollLines`.
 */
export function setViewportY(term: Terminal, vy: number): void {
  const maxVy = Math.max(0, term.buffer.active.length - term.rows);
  const target = Math.max(0, Math.min(maxVy, Math.round(vy)));
  const current = Math.round(term.getViewportY());
  if (current !== target) term.scrollLines(current - target);
}

/** Puts buffer line `index` about a third of the way down the viewport. */
export function scrollLineIntoView(term: Terminal, index: number): void {
  const maxVy = Math.max(0, term.buffer.active.length - term.rows);
  const top = Math.max(0, Math.min(maxVy, index - Math.floor(term.rows / 3)));
  setViewportY(term, maxVy - top);
}
