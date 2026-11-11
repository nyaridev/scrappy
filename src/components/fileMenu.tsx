// fileMenu: the right-click actions on a library file (open it, show it in the file manager) and on a link (copy
// it). Failures show as toasts.

import { Icon, type MenuEntry, toast } from "iro-ui";
import { CopyIcon, ExternalLinkIcon, FolderOpenIcon } from "iro-ui/icons";
import { openFile, showInFolder } from "../lib/backend";

// ── Public API ────────────────────────────
/** Shows a library file (a path relative to the library root) in the system file manager. */
export function showFile(path: string) {
  showInFolder(path).catch((error) =>
    toast({ tone: "error", title: "Couldn't show the file", description: String(error) }),
  );
}

/** Open and Show in folder for a library file; `open` replaces opening it in its default app. */
export function fileMenu(path: string, open?: () => void): MenuEntry[] {
  return [
    {
      label: "Open",
      icon: <Icon icon={ExternalLinkIcon} />,
      onSelect:
        open ??
        (() =>
          openFile(path).catch((error) =>
            toast({ tone: "error", title: "Couldn't open the file", description: String(error) }),
          )),
    },
    { label: "Show in folder", icon: <Icon icon={FolderOpenIcon} />, onSelect: () => showFile(path) },
  ];
}

/** Copy link, for a web address. */
export function copyLinkItem(url: string): MenuEntry {
  return {
    label: "Copy link",
    icon: <Icon icon={CopyIcon} />,
    onSelect: () =>
      navigator.clipboard
        .writeText(url)
        .catch((error) => toast({ tone: "error", title: "Couldn't copy the link", description: String(error) })),
  };
}
