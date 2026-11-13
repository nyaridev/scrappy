// MediaViewer: the Gallery's viewer over the views, under the header (ADR-0023): a picture whole, or a video or audio
// file in the player. ← and → (or the buttons at its sides) go to the previous or next file it can show; Escape or a
// click on the background around the file closes it. Its top bar names the file and where it is in the list, with Open
// in app, Show in folder and Close.

import { Icon, IconButton, toast } from "iro-ui";
import { ChevronLeftIcon, ChevronRightIcon, CloseIcon, ExternalLinkIcon, FolderOpenIcon } from "iro-ui/icons";
import { type MouseEvent, type PointerEvent, useEffect, useRef, useState } from "react";
import { showFile } from "../../components/fileMenu";
import { mediaUrl, openFile } from "../../lib/backend";
import type { GalleryItem } from "../../lib/types/GalleryItem";
import { PictureView } from "./PictureView";
import { VideoPlayer } from "./VideoPlayer";
import "./viewer.css";

// ── Constants ─────────────────────────────
/** How close to the end of the rows loaded the viewer asks the Gallery for more. */
const NEAR_END = 5;

// ── Types ─────────────────────────────────
interface Props {
  /** The Gallery's rows, as loaded. */
  items: GalleryItem[];
  /** The file shown. */
  path: string;
  onPathChange: (path: string) => void;
  onClose: () => void;
  /** Asks the Gallery for its next rows. */
  onNearEnd: () => void;
}

// ── Private helpers ───────────────────────
function open(path: string) {
  openFile(path).catch((error) =>
    toast({ tone: "error", title: "Couldn't open the file", description: String(error) }),
  );
}

// ── Public API ────────────────────────────
/** The viewer, open while it's shown. */
export function MediaViewer({ items, path, onPathChange, onClose, onNearEnd }: Props) {
  const root = useRef<HTMLDivElement>(null);
  // Whether the press started on the background, so a drag that ends there (selecting the name) doesn't close it.
  const pressedBackground = useRef(false);
  // The speed is kept from one video to the next while the viewer is open.
  const [speed, setSpeed] = useState(1);
  const shown = items.filter((item) => item.view != null);
  const at = shown.findIndex((item) => item.path === path);
  const item = shown[at];

  // Focus moves in, so keys reach the viewer, and goes back where it was on close.
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    root.current?.focus();
    return () => before?.focus();
  }, []);

  useEffect(() => {
    if (at >= shown.length - NEAR_END) onNearEnd();
  }, [at, shown.length, onNearEnd]);

  const go = (by: number) => {
    const next = shown[at + by];
    if (next) onPathChange(next.path);
  };

  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if ((event.target as HTMLElement | null)?.closest("[role=slider], input, [role=menu]")) return;
      if (event.key === "Escape" && !document.fullscreenElement) onClose();
      else if (event.key === "ArrowLeft") go(-1);
      else if (event.key === "ArrowRight") go(1);
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  });

  /** The viewer, its stage or the room around the file: not the file, the player or a button. */
  const isBackground = (target: EventTarget) =>
    target instanceof HTMLElement && target.matches(".viewer, .viewer-stage, .viewer-content");
  const press = (event: PointerEvent) => {
    pressedBackground.current = isBackground(event.target);
  };
  const click = (event: MouseEvent) => {
    if (pressedBackground.current && isBackground(event.target)) onClose();
  };

  if (!item) return null;
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape closes it, handled on the window above
    <div
      ref={root}
      className="viewer"
      role="dialog"
      aria-modal
      aria-label={item.name}
      tabIndex={-1}
      onPointerDown={press}
      onClick={click}
    >
      <div className="viewer-bar">
        <span className="viewer-name selectable">{item.name}</span>
        <span className="viewer-place">
          {at + 1} of {shown.length}
        </span>
        <IconButton icon={<Icon icon={ExternalLinkIcon} />} label="Open in app" onClick={() => open(item.path)} />
        <IconButton icon={<Icon icon={FolderOpenIcon} />} label="Show in folder" onClick={() => showFile(item.path)} />
        <IconButton icon={<Icon icon={CloseIcon} />} label="Close" shortcut="Esc" onClick={onClose} />
      </div>
      <div className="viewer-stage">
        <IconButton
          className="viewer-step"
          icon={<Icon icon={ChevronLeftIcon} />}
          label="Previous"
          shortcut="←"
          disabled={at === 0}
          onClick={() => go(-1)}
        />
        <div className="viewer-content">
          {item.view === "picture" ? (
            <PictureView key={item.path} path={item.path} name={item.name} onOpenInApp={() => open(item.path)} />
          ) : (
            <VideoPlayer
              key={item.path}
              src={mediaUrl(item.path)}
              name={item.name}
              audio={item.view === "audio"}
              speed={speed}
              onSpeedChange={setSpeed}
              onOpenInApp={() => open(item.path)}
            />
          )}
        </div>
        <IconButton
          className="viewer-step"
          icon={<Icon icon={ChevronRightIcon} />}
          label="Next"
          shortcut="→"
          disabled={at === shown.length - 1}
          onClick={() => go(1)}
        />
      </div>
    </div>
  );
}
