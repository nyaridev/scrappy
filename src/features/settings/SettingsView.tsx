// SettingsView: the library folder, how downloads run, the tools, the remembered sign-ins, the browser extension,
// Hunt's packs folder, the look, and which build is running.

import { Button, Field, Icon, NumberField, PropertyList, SectionTitle, Switch, toast } from "iro-ui";
import { DownloadIcon, FolderIcon, FolderOpenIcon, InfoIcon, PaletteIcon, SearchIcon, WrenchIcon } from "iro-ui/icons";
import { useSettings } from "../../app/SettingsProvider";
import { useCached } from "../../app/useCached";
import { useTools } from "../../app/useTools";
import { ToolList } from "../../components/ToolList";
import { appInfo, libraryStatus, openPacksFolder } from "../../lib/backend";
import { BrowserExtension } from "./BrowserExtension";
import { SignedInSites } from "./SignedInSites";

// ── Public API ────────────────────────────
/** Settings. */
export function SettingsView() {
  const { settings, saveLook, saveDownloads } = useSettings();
  const tools = useTools();
  const [info] = useCached("appInfo", appInfo);
  // Not taken from the shared settings, which were loaded before first-run setup saved the folder.
  const [library] = useCached("library", libraryStatus);
  const look = settings?.look;
  const limits = settings?.downloads;
  const root = library && library.state !== "notSet" ? library.root : "";

  return (
    <div className="settings">
      <section className="settings-section">
        <SectionTitle icon={<Icon icon={FolderIcon} />}>Library</SectionTitle>
        <PropertyList copyLabel="Copy" properties={[{ label: "Folder", value: root, mono: true, copy: root }]} />
      </section>

      <section className="settings-section">
        <SectionTitle icon={<Icon icon={DownloadIcon} />}>Downloads</SectionTitle>
        <div className="settings-fields">
          <Field label="Downloads at once">
            <NumberField
              value={limits?.atOnce ?? null}
              min={1}
              max={20}
              disabled={!limits}
              onValueChange={(atOnce) => limits && atOnce != null && saveDownloads({ ...limits, atOnce })}
            />
          </Field>
          <Field label="Speed limit per download (MB/s)" hint="0 for no limit.">
            <NumberField
              value={limits?.speedLimit ?? null}
              min={0}
              step={0.5}
              largeStep={5}
              format={{ maximumFractionDigits: 1 }}
              disabled={!limits}
              onValueChange={(speedLimit) => limits && saveDownloads({ ...limits, speedLimit: speedLimit ?? 0 })}
            />
          </Field>
        </div>
      </section>

      <section className="settings-section">
        <SectionTitle
          icon={<Icon icon={WrenchIcon} />}
          actions={
            (tools.updatable || tools.installing) && (
              <Button size="sm" icon={<Icon icon={DownloadIcon} />} loading={tools.installing} onClick={tools.install}>
                {tools.missing ? "Download missing" : "Update"}
              </Button>
            )
          }
        >
          Tools
        </SectionTitle>
        {tools.rows && <ToolList rows={tools.rows} />}
      </section>

      <SignedInSites />

      <BrowserExtension />

      <section className="settings-section">
        <SectionTitle
          icon={<Icon icon={SearchIcon} />}
          actions={
            <Button
              size="sm"
              icon={<Icon icon={FolderOpenIcon} />}
              onClick={() =>
                openPacksFolder().catch((error) =>
                  toast({ tone: "error", title: "Couldn't open the folder", description: String(error) }),
                )
              }
            >
              Open packs folder
            </Button>
          }
        >
          Hunt packs
        </SectionTitle>
        <p className="settings-hint">A pack file here with a built-in pack's name replaces it.</p>
      </section>

      <section className="settings-section">
        <SectionTitle icon={<Icon icon={PaletteIcon} />}>Look</SectionTitle>
        <Switch
          label="Liquid background"
          checked={look?.liquidBackground ?? false}
          disabled={!look}
          onCheckedChange={(on) => look && saveLook({ ...look, liquidBackground: on })}
        />
        <div className="settings-row">
          <Switch
            label="Use the system title bar"
            checked={look?.nativeFrame ?? false}
            disabled={!look}
            onCheckedChange={(on) => look && saveLook({ ...look, nativeFrame: on })}
          />
          <p className="settings-hint">Applies after a restart.</p>
        </div>
      </section>

      <section className="settings-section">
        <SectionTitle icon={<Icon icon={InfoIcon} />}>About</SectionTitle>
        <PropertyList
          properties={[
            { label: "Version", value: info?.version, mono: true },
            { label: "System", value: info?.os },
          ]}
        />
      </section>
    </div>
  );
}
