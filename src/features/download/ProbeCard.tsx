// ProbeCard: what Rust found at a link: thumbnail, title, channel, length and site; for a playlist, channel, post or
// profile, how many entries it holds and the first ones' names.

import { Icon, Image } from "iro-ui";
import { PlayIcon } from "iro-ui/icons";
import { formatCount, formatDuration } from "../../lib/format";
import type { Probe } from "../../lib/types/Probe";

/** The link's details. */
export function ProbeCard({ probe }: { probe: Probe }) {
  const count = probe.count != null ? formatCount(probe.count, probe.files ? "file" : "video", probe.more) : null;
  const facts = [count, probe.channel, probe.duration != null ? formatDuration(probe.duration) : null, probe.site];
  return (
    <div className="probe">
      {probe.thumbnail ? (
        <Image className="probe-thumbnail" src={probe.thumbnail} alt="" ratio={16 / 9} />
      ) : (
        <span className="probe-thumbnail probe-thumbnail-empty">
          <Icon icon={PlayIcon} size={24} />
        </span>
      )}
      <span className="probe-text">
        <span className="probe-title">{probe.title}</span>
        <span className="probe-facts">{facts.filter(Boolean).join(" · ")}</span>
        {probe.items.length > 0 && (
          <ul className="probe-items" aria-label="First entries">
            {probe.items.map((item, index) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: names can repeat, and the list never reorders
              <li key={index}>{item}</li>
            ))}
          </ul>
        )}
      </span>
    </div>
  );
}
