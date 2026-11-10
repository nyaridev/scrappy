// ScrapeView: placeholder until its milestone builds it.

import { EmptyState, Icon } from "iro-ui";
import { GlobeIcon } from "iro-ui/icons";

/** Scrape */
export function ScrapeView() {
  return (
    <EmptyState icon={<Icon icon={GlobeIcon} size={32} />} title="Scrape">
      Crawl sites, mirror them for offline use and hunt for images by topic. Arrives in M11 to M13.
    </EmptyState>
  );
}
