// StopButton: a running job's Stop (Hunt, Scrape, Grab, the duplicate finder). A job takes a moment to wind down, so
// the click shows at once: the button turns red and reads "Stopping" with a spinner until the job ends and the view
// takes it away. It stays enabled (iro-ui turns a disabled button gray) and ignores more clicks.

import { Button, Icon, Spinner } from "iro-ui";
import { StopIcon } from "iro-ui/icons";
import { useState } from "react";

/** Stop, then "Stopping" until the job ends. */
export function StopButton({ onStop, size }: { onStop: () => void; size?: "sm" | "md" }) {
  const [stopping, setStopping] = useState(false);
  return (
    <Button
      variant={stopping ? "danger" : "secondary"}
      size={size}
      icon={stopping ? <Spinner /> : <Icon icon={StopIcon} />}
      aria-busy={stopping || undefined}
      onClick={() => {
        if (stopping) return;
        setStopping(true);
        onStop();
      }}
    >
      {stopping ? "Stopping" : "Stop"}
    </Button>
  );
}
