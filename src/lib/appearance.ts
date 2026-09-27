// Ghostty appearance → terminal theme, font stack, and chrome colour variables.
import type { ITerminalOptions } from "ghostty-web";

export type TerminalTheme = NonNullable<ITerminalOptions["theme"]>;

export type GhosttyTheme = {
  background: string | null;
  foreground: string | null;
  cursor: string | null;
  selectionBackground: string | null;
  selectionForeground: string | null;
  palette: (string | null)[];
};

export type Appearance = {
  fontFamily: string[];
  fontSize: number | null;
  themeName: string | null;
  theme: GhosttyTheme | null;
  source: string | null;
};

export const DEFAULT_THEME: Required<
  Pick<
    TerminalTheme,
    | "background"
    | "foreground"
    | "cursor"
    | "selectionBackground"
    | "black"
    | "red"
    | "green"
    | "yellow"
    | "blue"
    | "magenta"
    | "cyan"
    | "white"
    | "brightBlack"
    | "brightRed"
    | "brightGreen"
    | "brightYellow"
    | "brightBlue"
    | "brightMagenta"
    | "brightCyan"
    | "brightWhite"
  >
> = {
  background: "#0f1115",
  foreground: "#d6d8de",
  cursor: "#0f1115",
  selectionBackground: "#2f3d5c",
  black: "#1a1d24",
  red: "#ff6b6b",
  green: "#7ee787",
  yellow: "#f2cc60",
  blue: "#79b8ff",
  magenta: "#d2a8ff",
  cyan: "#56d4dd",
  white: "#c9d1d9",
  brightBlack: "#6e7681",
  brightRed: "#ff8e8e",
  brightGreen: "#9cf2a6",
  brightYellow: "#ffdf80",
  brightBlue: "#a5d0ff",
  brightMagenta: "#e2c5ff",
  brightCyan: "#82e5ec",
  brightWhite: "#f0f3f6",
};

const PALETTE_KEYS = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

export type ResolvedTheme = typeof DEFAULT_THEME & TerminalTheme;

/** The cursor is hidden by painting it in the background colour: tiles are read-only. */
export function toTerminalTheme(a: Appearance | null): ResolvedTheme {
  const t = a?.theme;
  if (!t) return { ...DEFAULT_THEME };
  const out: ResolvedTheme = { ...DEFAULT_THEME };
  if (t.background) out.background = t.background;
  if (t.foreground) out.foreground = t.foreground;
  out.cursor = out.background;
  if (t.selectionBackground) out.selectionBackground = t.selectionBackground;
  if (t.selectionForeground) out.selectionForeground = t.selectionForeground;
  t.palette.forEach((color, i) => {
    const key = PALETTE_KEYS[i];
    if (color && key) out[key] = color;
  });
  return out;
}

export const DEFAULT_FONT_STACK = ['"JetBrains Mono"', '"SF Mono"', "Menlo", "monospace"];

export function fontFamilyCss(a: Appearance | null): string {
  const families = (a?.fontFamily ?? []).map((f) => `"${f.replace(/"/g, "")}"`);
  return [...families, ...DEFAULT_FONT_STACK].join(", ");
}

export const DEFAULT_FONT_SIZE = 14;
export const MIN_FONT_SIZE = 9;
export const MAX_FONT_SIZE = 32;

/** Ghostty sizes are points; on macOS those map 1:1 to CSS pixels. */
export function defaultFontSize(a: Appearance | null): number {
  const n = a?.fontSize ? Math.round(a.fontSize) : DEFAULT_FONT_SIZE;
  return clampFontSize(n);
}

export function clampFontSize(n: number): number {
  return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, Math.round(n)));
}

/** CSS custom properties so the app chrome follows the terminal theme. */
export function chromeVars(theme: ResolvedTheme, mono: string): Record<string, string> {
  return {
    "--term-bg": theme.background,
    "--term-fg": theme.foreground,
    "--term-red": theme.red,
    "--term-green": theme.green,
    "--term-yellow": theme.yellow,
    "--term-blue": theme.blue,
    "--term-magenta": theme.magenta,
    "--term-cyan": theme.cyan,
    "--term-dim": theme.brightBlack,
    "--mono": mono,
  };
}
