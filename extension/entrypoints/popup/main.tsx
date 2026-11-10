// Entry: loads the iro-ui style and mounts the popup inside IroRoot.

import "iro-ui/styles/sumi.css";
import "./popup.css";
import { IroRoot } from "iro-ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Popup } from "./Popup";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <IroRoot styleName="sumi" className="popup">
      <Popup />
    </IroRoot>
  </StrictMode>,
);
