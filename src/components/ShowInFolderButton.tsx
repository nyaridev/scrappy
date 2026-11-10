// ShowInFolderButton: opens the system file manager with a library file selected; a failure shows as a toast.

import { Icon, IconButton, toast } from "iro-ui";
import { FolderOpenIcon } from "iro-ui/icons";
import { showInFolder } from "../lib/backend";

/** The button, for a file path relative to the library root. */
export function ShowInFolderButton({ path }: { path: string }) {
  return (
    <IconButton
      size="sm"
      icon={<Icon icon={FolderOpenIcon} />}
      label="Show in folder"
      onClick={() =>
        showInFolder(path).catch((error) =>
          toast({ tone: "error", title: "Couldn't show the file", description: String(error) }),
        )
      }
    />
  );
}
