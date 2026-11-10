// Entry: loads the iro-ui style and mounts the app inside IroRoot.

import "iro-ui/styles/sumi.css";
import "./styles.css";
import { IroRoot, Toaster } from "iro-ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <IroRoot styleName="sumi" className="root">
      <App />
      <Toaster />
    </IroRoot>
  </StrictMode>,
);
