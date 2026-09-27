import ReactDOM from "react-dom/client";

// The same bundle serves two windows: the board, and the menu bar panel.
// Each is loaded on demand so the panel never pays for the terminal engine.
const view = new URLSearchParams(window.location.search).get("view");

void (async () => {
  const { default: Root } = view === "panel" ? await import("./PanelApp") : await import("./App");
  // No StrictMode: its double mount would spawn every source twice in dev.
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<Root />);
})();
