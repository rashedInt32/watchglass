import { useEffect, useMemo, useRef, useState } from "react";
import type { SearchHit, TileApi } from "./PaneTile";

type Props = {
  tiles: Map<string, TileApi>;
  order: string[];
  onJump: (hit: SearchHit) => void;
  onClose: () => void;
};

const LIMIT_PER_SOURCE = 40;

export function Palette({ tiles, order, onJump, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const hits = useMemo(() => {
    if (query.trim().length < 2) return [];
    const out: SearchHit[] = [];
    for (const id of order) {
      const api = tiles.get(id);
      if (api) out.push(...api.search(query, LIMIT_PER_SOURCE));
    }
    return out;
  }, [query, tiles, order]);

  useEffect(() => {
    setSelected(0);
  }, [hits.length]);

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(hits.length - 1, s + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(0, s - 1));
    } else if (e.key === "Enter") {
      const hit = hits[selected];
      if (hit) onJump(hit);
    } else if (e.key === "Escape") {
      onClose();
    }
  }

  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <div className="palette" onMouseDown={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Search every source…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKey}
          spellCheck={false}
        />
        <ul className="palette-list">
          {hits.map((hit, i) => (
            <li
              key={`${hit.id}:${hit.index}`}
              className={i === selected ? "palette-hit palette-hit--selected" : "palette-hit"}
              onMouseEnter={() => setSelected(i)}
              onClick={() => onJump(hit)}
            >
              <span className="palette-source">{hit.id}</span>
              <span className="palette-text">{hit.text}</span>
            </li>
          ))}
          {query.trim().length >= 2 && hits.length === 0 && (
            <li className="palette-empty">No matches in live scrollback.</li>
          )}
        </ul>
      </div>
    </div>
  );
}
