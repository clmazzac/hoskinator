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
import { Textarea } from "@/components/ui/textarea";
import { parseSheet } from "@/lib/sheet";
import { createApplication, linkSheet, sheetCsv, workspaceStatus } from "@/rpc";

export default function SheetImport({
  open,
  onOpenChange,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
}) {
  const [pasted, setPasted] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [linked, setLinked] = useState<string | null>(null);
  const [linkInput, setLinkInput] = useState("");
  const [linking, setLinking] = useState(false);
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    if (!open) return;
    workspaceStatus().then(
      (status) => setLinked(status.applications_sheet),
      () => setLinked(null),
    );
  }, [open]);

  const attempt = <T,>(
    setPending: (pending: boolean) => void,
    work: Promise<T>,
    onSuccess: (result: T) => void,
    onFailure?: () => void,
  ) => {
    setPending(true);
    setError(null);
    work.then(
      (result) => {
        setPending(false);
        onSuccess(result);
      },
      (failure: Error) => {
        setPending(false);
        setError(failure.message);
        onFailure?.();
      },
    );
  };

  const link = () =>
    attempt(setLinking, linkSheet(linkInput), (status) => {
      setLinkInput("");
      setLinked(status.applications_sheet);
    });

  const sync = () => attempt(setSyncing, sheetCsv(), setPasted);

  const rows = pasted.trim() === "" ? [] : parseSheet(pasted);

  const bring = () =>
    attempt(
      setBusy,
      rows.reduce(
        (queue, row) => queue.then(() => createApplication(row).then(() => undefined)),
        Promise.resolve(),
      ),
      () => {
        setPasted("");
        onOpenChange(false);
        onImported();
      },
      onImported,
    );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Import from a sheet</DialogTitle>
        </DialogHeader>

        <div className="grid gap-1.5">
          <Label className="text-xs">Linked sheet</Label>
          {linked ? (
            <div className="flex items-center gap-2">
              <a
                href={`https://docs.google.com/spreadsheets/d/${linked}/edit`}
                target="_blank"
                rel="noreferrer"
                className="truncate font-mono text-xs text-muted-foreground underline underline-offset-2"
              >
                {linked}
              </a>
              <Button
                size="sm"
                variant="outline"
                className="h-7 shrink-0 text-xs"
                disabled={syncing}
                onClick={sync}
              >
                {syncing ? "Syncing…" : "Sync now"}
              </Button>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <Input
                value={linkInput}
                onChange={(event) => setLinkInput(event.target.value)}
                placeholder="Paste a Google Sheets link"
                className="h-7 text-xs"
                spellCheck={false}
              />
              <Button
                size="sm"
                className="h-7 shrink-0 text-xs"
                disabled={linking || linkInput.trim() === ""}
                onClick={link}
              >
                {linking ? "Linking…" : "Link"}
              </Button>
            </div>
          )}
          <p className="text-[11px] text-muted-foreground">
            Must be shared "Anyone with the link" as a viewer. Read-only — writes still go through
            Export.
          </p>
        </div>

        <p className="text-xs text-muted-foreground">
          Or select the rows in Google Sheets, including the heading row, and paste. Columns
          are matched by their headings.
        </p>

        <Textarea
          value={pasted}
          onChange={(event) => setPasted(event.target.value)}
          placeholder="Company	Position	Date Applied	Application Status	Listing Page	Resume	Notes"
          className="h-48 font-mono text-xs"
          spellCheck={false}
        />

        {rows.length > 0 && (
          <div className="max-h-40 overflow-auto rounded-md border">
            <table className="w-full text-xs">
              <tbody>
                {rows.slice(0, 8).map((row, index) => (
                  <tr key={index} className="border-b last:border-b-0">
                    <td className="px-2 py-1 font-medium">{row.company}</td>
                    <td className="truncate px-2 py-1 text-muted-foreground">
                      {row.position}
                    </td>
                    <td className="px-2 py-1 text-muted-foreground tabular-nums">
                      {row.date_applied}
                    </td>
                    <td className="px-2 py-1 text-muted-foreground">{row.status}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {error && (
          <p className="font-mono text-xs whitespace-pre-wrap text-destructive">{error}</p>
        )}

        <DialogFooter>
          <span className="mr-auto self-center text-xs text-muted-foreground tabular-nums">
            {rows.length} row{rows.length === 1 ? "" : "s"} ready
          </span>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button disabled={busy || rows.length === 0} onClick={bring}>
            {busy ? "Importing…" : "Import"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
