// WindowControls: minimize, maximize or restore, and close, for the frameless window (ADR-0011).

import { Icon, IconButton } from "iro-ui";
import { CloseIcon, ExpandIcon, MinusIcon, ShrinkIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { closeWindow, minimizeWindow, toggleMaximizeWindow, watchMaximized } from "../lib/backend";

/** The window buttons at the header's right end, where Windows and Linux put them. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => watchMaximized(setMaximized), []);

  return (
    <div className="window-controls">
      <IconButton icon={<Icon icon={MinusIcon} />} label="Minimize" onClick={minimizeWindow} />
      <IconButton
        icon={<Icon icon={maximized ? ShrinkIcon : ExpandIcon} />}
        label={maximized ? "Restore" : "Maximize"}
        onClick={toggleMaximizeWindow}
      />
      <IconButton className="window-close" icon={<Icon icon={CloseIcon} />} label="Close" onClick={closeWindow} />
    </div>
  );
}
