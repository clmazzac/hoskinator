import { useState } from "react";

import { Button } from "@/components/ui/button";
import { renderPreview } from "@/rpc";
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

// rendercv 2.8 writes PDF, HTML, Markdown, PNG and Typst — never DOCX, and nothing here
// converts to it. PDF is the only one offered until a second format is asked for.
const FORMATS = [{ value: "pdf", label: "PDF", extension: "pdf" }];

export default function ExportDialog({
  open,
  onOpenChange,
  onRender,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRender: () => void;
}) {
  const [name, setName] = useState("resume");
  const [busy, setBusy] = useState(false);
  const [format, setFormat] = useState("pdf");
  const extension =
    FORMATS.find((candidate) => candidate.value === format)?.extension ?? "pdf";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Export</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <div className="grid gap-1.5">
            <Label htmlFor="export-name">File name</Label>
            <div className="flex items-center gap-2">
              <Input
                id="export-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                spellCheck={false}
              />
              <span className="text-sm text-muted-foreground tabular-nums">
                .{extension}
              </span>
            </div>
          </div>

          <div className="grid gap-1.5">
            <Label htmlFor="export-format">Format</Label>
            <Select value={format} onValueChange={(value) => value && setFormat(value)}>
              <SelectTrigger id="export-format">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {FORMATS.map((candidate) => (
                  <SelectItem key={candidate.value} value={candidate.value}>
                    {candidate.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button
            disabled={busy || name.trim() === ""}
            onClick={() => {
              setBusy(true);
              renderPreview().then(
                () => {
                  setBusy(false);
                  onOpenChange(false);
                  onRender();
                  window.location.href = `/preview.pdf?download=${encodeURIComponent(
                    `${name}.${extension}`,
                  )}`;
                },
                () => setBusy(false),
              );
            }}
          >
            {busy ? "Rendering…" : "Export"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
