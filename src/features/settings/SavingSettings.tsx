// SavingSettings: what a download saves and where (ADR-0034): the preset and quality the Download view starts on, a
// single download's name with a preview, subtitles and their languages, and what's embedded into the file.

import { Field, Icon, Panel, SectionTitle, Select, Switch } from "iro-ui";
import { SaveIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { useCached } from "../../app/useCached";
import { nameExample, presets } from "../../lib/backend";
import type { MediaOptions } from "../../lib/types/MediaOptions";
import { TextSetting } from "./TextSetting";

// ── Constants ─────────────────────────────
const BEST = "best";
const QUALITIES = [
  { value: BEST, label: "Best" },
  ...[2160, 1440, 1080, 720, 480, 360].map((height) => ({ value: String(height), label: `${height}p` })),
];
const FIELDS = "{title} {id} {channel} {upload_date} {site} {user}";

// ── Public API ────────────────────────────
/** The Saving section of Settings. */
export function SavingSettings() {
  const { settings, savePreferences } = useSettings();
  const [allPresets] = useCached("presets", presets);
  const [example, setExample] = useState<string | null>(null);
  const preferences = settings?.preferences;
  const media = preferences?.media;
  const template = media?.nameTemplate;

  useEffect(() => {
    if (template) void nameExample(template).then(setExample, () => setExample(null));
  }, [template]);

  if (!preferences || !media) return null;
  const save = (part: Partial<MediaOptions>) => savePreferences({ ...preferences, media: { ...media, ...part } });
  const toggle = (part: Partial<MediaOptions>) => void save(part);

  return (
    <Panel as="section" className="settings-section">
      <SectionTitle icon={<Icon icon={SaveIcon} />}>Saving</SectionTitle>
      <div className="settings-fields">
        <Field label="Start with preset">
          <Select
            options={(allPresets ?? []).map((preset) => ({ value: preset.id, label: preset.name }))}
            value={media.preset ?? allPresets?.[0]?.id ?? null}
            onValueChange={(preset) => toggle({ preset })}
          />
        </Field>
        <Field label="Start with quality" hint="Or the next lower one a video has.">
          <Select
            options={QUALITIES}
            value={media.maxHeight == null ? BEST : String(media.maxHeight)}
            onValueChange={(value) => toggle({ maxHeight: value == null || value === BEST ? null : Number(value) })}
          />
        </Field>
      </div>
      <TextSetting
        label="File name"
        value={media.nameTemplate}
        onSave={(nameTemplate) => save({ nameTemplate })}
        hint={
          <>
            <span className="selectable">{example ?? ""}</span>
            <br />
            {FIELDS}. Playlists keep a folder each.
          </>
        }
      />
      <div className="settings-switches">
        <Switch label="Subtitles" checked={media.subtitles} onCheckedChange={(subtitles) => toggle({ subtitles })} />
        <Switch
          label="Embed metadata"
          checked={media.embedMetadata}
          onCheckedChange={(embedMetadata) => toggle({ embedMetadata })}
        />
        <Switch
          label="Embed thumbnail"
          checked={media.embedThumbnail}
          onCheckedChange={(embedThumbnail) => toggle({ embedThumbnail })}
        />
        <Switch
          label="Embed chapters"
          checked={media.embedChapters}
          onCheckedChange={(embedChapters) => toggle({ embedChapters })}
        />
      </div>
      {media.subtitles && (
        <div className="settings-fields">
          <TextSetting
            label="Subtitle languages"
            hint="Codes like en or en,pl. Embedded into the video."
            value={media.subtitleLanguages}
            onSave={(subtitleLanguages) => save({ subtitleLanguages })}
          />
          <Switch
            label="Auto-generated too"
            checked={media.autoSubtitles}
            onCheckedChange={(autoSubtitles) => toggle({ autoSubtitles })}
          />
        </div>
      )}
    </Panel>
  );
}
