import { useState } from "react";

import EntryTypeSelect from "@/components/EntryTypeSelect";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { ENTRY_FIELDS, TEXT_FIELD, buildFields, isListField } from "@/entryFields";
import { push } from "@/lib/history";
import { ENTRY_TYPES, createEntry, deleteEntry } from "@/rpc";

/// Quick-creates an Entry in the Master Store, the way an IDE's "New File" starts a blank one.
export default function NewEntryDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [entryType, setEntryType] = useState<string>(ENTRY_TYPES[0]);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const write = (name: string, written: string) => setValues({ ...values, [name]: written });

  const retype = (chosen: string) => {
    setEntryType(chosen);
    setValues({});
  };

  const create = () => {
    setBusy(true);
    setError(null);
    const fields = buildFields(entryType, values);
    createEntry(entryType, fields).then(
      (created) => {
        push({
          undo: () => deleteEntry(created.id),
          redo: () => Promise.resolve(created),
          kind: "store",
        });
        setBusy(false);
        setValues({});
        onOpenChange(false);
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>New entry</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <EntryTypeSelect id="new-entry-type" value={entryType} onChange={retype} />

          {entryType === "text" ? (
            <div className="grid gap-1.5">
              <Label htmlFor="new-entry-text" className="text-xs">
                Text
              </Label>
              <Textarea
                id="new-entry-text"
                rows={4}
                value={values[TEXT_FIELD] ?? ""}
                spellCheck={false}
                onChange={(event) => write(TEXT_FIELD, event.target.value)}
              />
            </div>
          ) : (
            ENTRY_FIELDS[entryType].map((name) => (
              <div key={name} className="grid gap-1.5">
                <Label htmlFor={`new-entry-${name}`} className="text-xs">
                  {name}
                  {isListField(name) ? " (one per line)" : ""}
                </Label>
                {isListField(name) ? (
                  <Textarea
                    id={`new-entry-${name}`}
                    rows={3}
                    value={values[name] ?? ""}
                    spellCheck={false}
                    onChange={(event) => write(name, event.target.value)}
                  />
                ) : (
                  <Input
                    id={`new-entry-${name}`}
                    value={values[name] ?? ""}
                    onChange={(event) => write(name, event.target.value)}
                  />
                )}
              </div>
            ))
          )}
        </div>

        {error && (
          <p className="text-xs text-destructive" role="alert">
            {error}
          </p>
        )}

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button disabled={busy} onClick={create}>
            {busy ? "Creating…" : "Create"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
