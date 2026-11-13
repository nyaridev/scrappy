// PictureView: a library picture whole in the Gallery's viewer, read from Rust as a `data:` URL; one the window can't
// show offers Open in app.

import { Button, Spinner } from "iro-ui";
import { useEffect, useState } from "react";
import { picture } from "../../lib/backend";

// ── Public API ────────────────────────────
/** The picture at `path`, fitted whole. */
export function PictureView({ path, name, onOpenInApp }: { path: string; name: string; onOpenInApp: () => void }) {
  // `undefined` while it loads, `null` when the window can't show it.
  const [src, setSrc] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let current = true;
    setSrc(undefined);
    picture(path)
      .catch(() => null)
      .then((read) => current && setSrc(read));
    return () => {
      current = false;
    };
  }, [path]);

  if (src === undefined) return <Spinner label="Loading the picture" />;
  if (src === null) {
    return (
      <div className="player-failed">
        <span>This picture can't show here.</span>
        <Button onClick={onOpenInApp}>Open in app</Button>
      </div>
    );
  }
  return <img className="viewer-picture" src={src} alt={name} />;
}
