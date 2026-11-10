// GalleryView: placeholder until its milestone builds it.

import { EmptyState, Icon } from "iro-ui";
import { ImagesIcon } from "iro-ui/icons";

/** Gallery */
export function GalleryView() {
  return (
    <EmptyState icon={<Icon icon={ImagesIcon} size={32} />} title="Gallery">
      Browse and view everything you've downloaded. Arrives in M9.
    </EmptyState>
  );
}
