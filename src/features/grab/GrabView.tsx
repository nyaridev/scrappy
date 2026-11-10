// GrabView: placeholder until its milestone builds it.

import { EmptyState, Icon } from "iro-ui";
import { TargetIcon } from "iro-ui/icons";

/** Grab */
export function GrabView() {
  return (
    <EmptyState icon={<Icon icon={TargetIcon} size={32} />} title="Grab">
      Find every video, stream and image on a page and pick what to keep. Arrives in M8.
    </EmptyState>
  );
}
