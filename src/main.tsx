import ReactDOM from "react-dom/client";
import App from "./App";

// No StrictMode: its double mount would spawn every source twice in dev.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
