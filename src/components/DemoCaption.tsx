import { useEffect, useState } from "react";

/**
 * Big caption the demo recorder drives through `window.__wg.caption(text)`.
 * Only mounted in mock mode; invisible until a caption is set.
 */
export function DemoCaption() {
  const [text, setText] = useState<string | null>(null);
  useEffect(() => {
    const w = window as unknown as { __wg?: { caption?: (t: string | null) => void } };
    w.__wg ??= {};
    w.__wg.caption = (t) => setText(t);
    return () => {
      if (w.__wg) delete w.__wg.caption;
    };
  }, []);
  if (!text) return null;
  return (
    <div className="caption" key={text}>
      {text}
    </div>
  );
}
