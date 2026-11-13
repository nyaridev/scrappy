// PackEditor: the Hunt packs dialog. A picker of every pack (hidden ones marked) and New pack; for the picked pack its
// name (a built-in pack's can't change), whether it shows in Hunt's Pack list, whether it searches any site (web
// search), and its sites, each switched on or off or removed, and new ones added; Delete for the user's own packs.
// Rust checks a pack and saves it as the user's (ADR-0020, ADR-0028).

import { AlertDialog, Button, Dialog, Field, Icon, IconButton, Notice, SingleSelect, Switch, TextField } from "iro-ui";
import { PlusIcon, TrashIcon } from "iro-ui/icons";
import { useState } from "react";
import { createPack, huntPacks, removePack, savePack } from "../../lib/backend";
import type { Pack } from "../../lib/types/Pack";
import type { Packs } from "../../lib/types/Packs";

// ── Constants ─────────────────────────────
/** The heading of the user's own packs that name none. */
export const YOUR_PACKS = "Your packs";

// ── Types ─────────────────────────────────
interface Props {
  packs: Pack[];
  /** The pack it opens on: the one Hunt has picked. */
  first: string;
  /** Ids of the packs left out of the Pack list. */
  hidden: string[];
  onHidden: (ids: string[]) => void;
  onClose: () => void;
  /** Every pack, read again after a save, and the saved pack's id; `created` for a new one. */
  onSaved: (packs: Packs, id: string, created: boolean) => void;
  /** Every pack, read again after a removal. */
  onRemoved: (packs: Packs) => void;
}

// ── Public API ────────────────────────────
/** The packs dialog, open while it's shown. */
export function PackEditor({ packs, first, hidden, onHidden, onClose, onSaved, onRemoved }: Props) {
  // The pack being edited; `null` for a new one.
  const [pack, setPack] = useState<Pack | null>(packs.find((one) => one.id === first) ?? packs[0] ?? null);
  const [name, setName] = useState(pack?.name ?? "");
  const [shown, setShown] = useState(pack ? !hidden.includes(pack.id) : true);
  const [web, setWeb] = useState(pack?.web ?? false);
  const [sites, setSites] = useState(pack?.sites ?? []);
  const [off, setOff] = useState(pack?.off ?? []);
  const [typed, setTyped] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  // The last shown pack can't be hidden: the Pack list would be empty.
  const othersShown = packs.some((one) => one.id !== pack?.id && !hidden.includes(one.id));

  /** Starts editing `next` (`null`: a new pack), dropping unsaved changes. */
  const open = (next: Pack | null) => {
    setPack(next);
    setName(next?.name ?? "");
    setShown(next ? !hidden.includes(next.id) : true);
    setWeb(next?.web ?? false);
    setSites(next?.sites ?? []);
    setOff(next?.off ?? []);
    setTyped("");
    setError(null);
  };

  const add = () => {
    const site = typed.trim();
    if (!site) return;
    if (!sites.includes(site)) setSites([...sites, site]);
    setTyped("");
  };

  const remove = (site: string) => {
    setSites(sites.filter((one) => one !== site));
    setOff(off.filter((one) => one !== site));
  };

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const id = pack ? pack.id : await createPack({ id: "", name, group: null, web, sites, off, builtIn: false });
      const read = pack ? await savePack({ ...pack, name, web, sites, off }) : await huntPacks();
      const others = hidden.filter((one) => one !== id);
      if (shown !== !hidden.includes(id)) onHidden(shown ? others : [...others, id]);
      onSaved(read, id, pack == null);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const deletePack = async () => {
    if (!pack) return;
    try {
      onRemoved(await removePack(pack.id));
    } catch (e) {
      setError(String(e));
      setDeleting(false);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(isOpen) => !isOpen && onClose()}
      title={pack ? "Edit pack" : "New pack"}
      footer={
        <>
          {pack && !pack.builtIn && (
            <Button
              className="pack-delete"
              variant="danger"
              icon={<Icon icon={TrashIcon} />}
              onClick={() => setDeleting(true)}
            >
              Delete
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={saving} onClick={save}>
            Save
          </Button>
        </>
      }
    >
      <div className="pack-editor">
        <div className="pack-pick">
          <SingleSelect
            className="pack-pick-select"
            aria-label="Pack"
            placeholder="New pack"
            options={packs.map((one) => ({
              value: one.id,
              label: one.name,
              group: one.group ?? YOUR_PACKS,
              hint: hidden.includes(one.id) ? "Hidden" : undefined,
            }))}
            value={pack?.id ?? null}
            onValueChange={(id) => open(packs.find((one) => one.id === id) ?? null)}
          />
          <Button icon={<Icon icon={PlusIcon} />} disabled={pack == null} onClick={() => open(null)}>
            New pack
          </Button>
        </div>
        {error && (
          <Notice tone="danger" title="Couldn't save the pack">
            <span className="selectable">{error}</span>
          </Notice>
        )}
        <Field label="Name" hint={pack?.builtIn ? "Built-in packs keep their name." : undefined}>
          <TextField
            value={name}
            onChange={(event) => setName(event.target.value)}
            disabled={pack?.builtIn}
            autoComplete="off"
          />
        </Field>
        <Switch
          label="Show in the Pack list"
          checked={shown}
          disabled={shown && !othersShown}
          onCheckedChange={setShown}
        />
        <Switch label="Any site (web search)" checked={web} onCheckedChange={setWeb} />
        <Field label="Sites">
          {sites.length === 0 ? (
            <span className="pack-empty">No sites</span>
          ) : (
            <ul className="pack-sites">
              {sites.map((site) => (
                <li key={site} className="pack-site">
                  <Switch
                    size="sm"
                    label={site}
                    checked={!off.includes(site)}
                    onCheckedChange={(on) => setOff(on ? off.filter((one) => one !== site) : [...off, site])}
                  />
                  <IconButton
                    size="sm"
                    icon={<Icon icon={TrashIcon} />}
                    label={`Remove ${site}`}
                    onClick={() => remove(site)}
                  />
                </li>
              ))}
            </ul>
          )}
        </Field>
        <Field label="Add a site">
          <TextField
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && add()}
            placeholder="example.com"
            spellCheck={false}
            autoComplete="off"
            end={<IconButton icon={<Icon icon={PlusIcon} />} label="Add site" disabled={!typed.trim()} onClick={add} />}
          />
        </Field>
      </div>
      <AlertDialog
        open={deleting}
        onOpenChange={setDeleting}
        title={`Delete ${pack?.name ?? "this pack"}?`}
        description="Its file is deleted. This can't be undone."
        confirmLabel="Delete"
        onConfirm={deletePack}
      />
    </Dialog>
  );
}
