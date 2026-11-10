// HomeView: placeholder until its milestone builds it.

import { EmptyState, Icon } from "iro-ui";
import { HomeIcon } from "iro-ui/icons";

/** Home */
export function HomeView() {
  return (
    <EmptyState icon={<Icon icon={HomeIcon} size={32} />} title="Home">
      Download stats, recent downloads and a shortcut to your folder arrive in M4.
    </EmptyState>
  );
}
