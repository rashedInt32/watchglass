// Dev-only handles for probing terminals from the browser console or scripts.
import type { Terminal } from "ghostty-web";

type Registry = { terms: Record<string, Terminal> };

function registry(): Registry | null {
  if (!import.meta.env.DEV || typeof window === "undefined") return null;
  const w = window as unknown as { __wg?: Partial<Registry> };
  // Other dev helpers (the demo caption) share this object; never assume its shape.
  w.__wg ??= {};
  w.__wg.terms ??= {};
  return w.__wg as Registry;
}

export function registerTerminal(name: string, term: Terminal | null): void {
  const r = registry();
  if (!r) return;
  if (term) r.terms[name] = term;
  else delete r.terms[name];
}
