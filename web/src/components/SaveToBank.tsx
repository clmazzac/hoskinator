import { useEffect, useState } from "react";

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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { entryLabel } from "@/entryFields";
import {
  createBullet,
  createVariant,
  listBullets,
  listEntries,
  type Bullet,
  type Entry,
} from "@/rpc";

/// Saves a wording from a resume back into the Master Store.
///
/// The one write that flows the other way (ADR-0001), and it is always explicit: the store and
/// the repository share no key, so which Bullet a wording belongs to is a question only the
/// person editing can answer.
export default function SaveToBank({
  text,
  open,
  onOpenChange,
  onSaved,
}: {
  text: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved?: () => void;
}) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [entryId, setEntryId] = useState<string>("");
  const [bullets, setBullets] = useState<Bullet[]>([]);
  const [bulletId, setBulletId] = useState<string>("new");
  const [wording, setWording] = useState(text);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => setWording(text), [text]);

  useEffect(() => {
    if (!open) return;
    listEntries().then(
      (loaded) => setEntries(loaded.filter((entry) => carries(entry.entry_type))),
      (failure: Error) => setError(failure.message),
    );
  }, [open]);

  useEffect(() => {
    if (!entryId) return setBullets([]);
    listBullets(Number(entryId)).then(setBullets, () => setBullets([]));
    setBulletId("new");
  }, [entryId]);

  const save = () => {
    setBusy(true);
    setError(null);
    const written = wording.trim();
    const kept = note.trim() || null;
    const work =
      bulletId === "new"
        ? createBullet(Number(entryId), written, kept)
        : createVariant(Number(bulletId), written, kept);

    work.then(
      () => {
        setBusy(false);
        setNote("");
        onOpenChange(false);
        onSaved?.();
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Save this wording to the bank</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <div className="grid gap-1.5">
            <Label className="text-xs">Wording</Label>
            <Textarea
              value={wording}
              onChange={(event) => setWording(event.target.value)}
              className="h-20 text-xs"
            />
          </div>

          <div className="grid gap-1.5">
            <Label className="text-xs">Entry it belongs to</Label>
            <Select value={entryId} onValueChange={(next) => next && setEntryId(next)}>
              <SelectTrigger>
                <SelectValue placeholder="Choose an entry" />
              </SelectTrigger>
              <SelectContent>
                {entries.map((entry) => {
                  const { title, subtitle } = entryLabel(entry.entry_type, entry.fields);
                  return (
                    <SelectItem key={entry.id} value={String(entry.id)}>
                      {title}
                      {subtitle ? ` · ${subtitle}` : ""}
                    </SelectItem>
                  );
                })}
              </SelectContent>
            </Select>
          </div>

          {entryId && (
            <div className="grid gap-1.5">
              <Label className="text-xs">Accomplishment</Label>
              <Select value={bulletId} onValueChange={(next) => next && setBulletId(next)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="new">A new accomplishment</SelectItem>
                  {bullets.map((bullet) => {
                    const shown =
                      bullet.variants.find((variant) => variant.is_default) ??
                      bullet.variants[0];
                    return (
                      <SelectItem key={bullet.id} value={String(bullet.id)}>
                        {shown?.text.slice(0, 70)}
                        {(shown?.text.length ?? 0) > 70 ? "…" : ""}
                      </SelectItem>
                    );
                  })}
                </SelectContent>
              </Select>
              <p className="text-[11px] text-muted-foreground">
                A new accomplishment starts its own Bullet. Choosing one already there adds
                this as another wording of it.
              </p>
            </div>
          )}

          <div className="grid gap-1.5">
            <Label htmlFor="variant-note" className="text-xs">
              Note
            </Label>
            <Input
              id="variant-note"
              value={note}
              onChange={(event) => setNote(event.target.value)}
              placeholder="punchy, metrics-forward"
            />
          </div>
        </div>

        {error && (
          <p className="font-mono text-xs whitespace-pre-wrap text-destructive">{error}</p>
        )}

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button disabled={busy || !entryId || wording.trim() === ""} onClick={save}>
            {busy ? "Saving…" : "Save to bank"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function carries(entryType: string): boolean {
  return ["normal", "experience", "education"].includes(entryType);
}
