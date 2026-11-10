// DownloadView: placeholder until its milestone builds it.

import { EmptyState, Icon } from "iro-ui";
import { DownloadIcon } from "iro-ui/icons";

/** Download */
export function DownloadView() {
  return (
    <EmptyState icon={<Icon icon={DownloadIcon} size={32} />} title="Download">
      Paste a YouTube, X, Instagram or other link and pick quality and format. Arrives in M3.
    </EmptyState>
  );
}
