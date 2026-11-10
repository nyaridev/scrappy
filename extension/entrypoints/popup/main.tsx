// Popup: placeholder until M5, when it lists the page's videos and images and sends a pick to the app.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <main style={{ width: 280, padding: 16, fontFamily: "system-ui, sans-serif" }}>
      <h1 style={{ margin: 0, fontSize: 16 }}>Scrappy</h1>
      <p>This page's videos and images will be listed here, ready to send to the app.</p>
    </main>
  </StrictMode>,
);
