// WindowControls: minimize, maximize or restore, and close, for the frameless window (ADR-0011). The window glyphs
// (a line, a square, two squares) are drawn here: iro-ui has none.

import { Icon, IconButton } from "iro-ui";
import { CloseIcon, icon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { closeWindow, minimizeWindow, toggleMaximizeWindow, watchMaximized } from "../lib/backend";

// ── Constants ─────────────────────────────
const path = (d: string) => `<path d="${d}"/>`;
const MinimizeIcon = icon("window-minimize", { bold: path("M6 12h12") });
const MaximizeIcon = icon("window-maximize", {
  bold: path("M8 6h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2Z"),
});
const RestoreIcon = icon("window-restore", {
  bold:
    path("M7 9h6a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2Z") +
    path("M9 9V7a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-2"),
});

// ── Public API ────────────────────────────
/** The window buttons at the header's right end, where Windows and Linux put them. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => watchMaximized(setMaximized), []);

  return (
    <div className="window-controls">
      <IconButton icon={<Icon icon={MinimizeIcon} />} label="Minimize" onClick={minimizeWindow} />
      <IconButton
        icon={<Icon icon={maximized ? RestoreIcon : MaximizeIcon} />}
        label={maximized ? "Restore" : "Maximize"}
        onClick={toggleMaximizeWindow}
      />
      <IconButton className="window-close" icon={<Icon icon={CloseIcon} />} label="Close" onClick={closeWindow} />
    </div>
  );
}
