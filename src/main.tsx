import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";

async function applyTauriWindowIcon() {
  try {
    const [{ defaultWindowIcon }, { getCurrentWindow }] = await Promise.all([
      import("@tauri-apps/api/app"),
      import("@tauri-apps/api/window"),
    ]);
    const window = getCurrentWindow();
    try {
      await window.setIcon("/favicon.ico");
      return;
    } catch {
      // Fall back to the bundled icon when the web asset path is unavailable.
    }

    const bundledIcon = await defaultWindowIcon();
    if (bundledIcon) {
      await window.setIcon(bundledIcon);
    }
  } catch {
    // Browser-only dev and preview builds do not expose Tauri's window APIs.
  }
}

void applyTauriWindowIcon();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
