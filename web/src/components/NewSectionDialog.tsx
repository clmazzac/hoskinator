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
import { push } from "@/lib/history";
import { ENTRY_TYPES, createSection, deleteSection } from "@/rpc";

/// Quick-creates a Section in the Master Store, the way NewEntryDialog quick-creates an Entry.
export default function NewSectionDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [name, setName] = useState("");
  const [entryType, setEntryType] = useState<string>(ENTRY_TYPES[0]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const create = () => {
    setBusy(true);
    setError(null);
    createSection(name, entryType).then(
      (created) => {
        push({
          undo: () => deleteSection(created.name),
          redo: () => Promise.resolve(created),
          kind: "store",
        });
        setBusy(false);
        setName("");
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
          <DialogTitle>New section</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <div className="grid gap-1.5">
            <Label htmlFor="new-section-name" className="text-xs">
              Name
            </Label>
            <Input
              id="new-section-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </div>

          <EntryTypeSelect
            id="new-section-type"
            label="Entry type"
            value={entryType}
            onChange={setEntryType}
          />
        </div>

        {error && (
          <p className="text-xs text-destructive" role="alert">
            {error}
          </p>
        )}

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button disabled={busy || !name.trim()} onClick={create}>
            {busy ? "Creating…" : "Create"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
