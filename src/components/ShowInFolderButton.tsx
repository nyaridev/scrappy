// ShowInFolderButton: opens the system file manager with a library file selected; a failure shows as a toast.

import { Icon, IconButton } from "iro-ui";
import { FolderOpenIcon } from "iro-ui/icons";
import { showFile } from "./fileMenu";

/** The button, for a file path relative to the library root. */
export function ShowInFolderButton({ path }: { path: string }) {
  return (
    <IconButton size="sm" icon={<Icon icon={FolderOpenIcon} />} label="Show in folder" onClick={() => showFile(path)} />
  );
}
