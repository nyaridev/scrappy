// TextSetting: a text setting saved when it loses focus or Enter is pressed, showing why Rust refused it under it.

import { Field, TextField } from "iro-ui";
import { type ReactNode, useState } from "react";

// ── Public API ────────────────────────────
/** A labelled text field for one setting; `onSave` resolves with the reason it wasn't saved, or `null`. */
export function TextSetting({
  label,
  hint,
  value,
  placeholder,
  onSave,
}: {
  label: string;
  hint?: ReactNode;
  value: string;
  placeholder?: string;
  onSave: (text: string) => Promise<string | null>;
}) {
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | null>(null);
  const [shown, setShown] = useState(value);

  // A value saved elsewhere (or brought into range by Rust) replaces the draft.
  if (value !== shown) {
    setShown(value);
    setDraft(value);
  }

  const commit = async () => {
    if (draft === value) {
      setError(null);
      return;
    }
    setError(await onSave(draft));
  };

  return (
    <Field label={label} hint={hint} error={error ?? undefined}>
      <TextField
        value={draft}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void commit()}
        onKeyDown={(event) => event.key === "Enter" && void commit()}
      />
    </Field>
  );
}
