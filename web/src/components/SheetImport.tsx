import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { parseSheet } from "@/lib/sheet";
import { createApplication } from "@/rpc";

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

  const rows = pasted.trim() === "" ? [] : parseSheet(pasted);

  const bring = () => {
    setBusy(true);
    setError(null);
    rows
      .reduce(
        (queue, row) => queue.then(() => createApplication(row).then(() => undefined)),
        Promise.resolve(),
      )
      .then(
        () => {
          setBusy(false);
          setPasted("");
          onOpenChange(false);
          onImported();
        },
        (failure: Error) => {
          setBusy(false);
          setError(failure.message);
          onImported();
        },
      );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Import from a sheet</DialogTitle>
        </DialogHeader>

        <p className="text-xs text-muted-foreground">
          Select the rows in Google Sheets, including the heading row, and paste. Columns
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
