import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { renderAvailableDocx, renderPreview, renderPreviewDocx } from "@/rpc";
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

// DOCX goes through pandoc, over the Markdown rendercv writes (rendercv has no native DOCX
// writer). Both must be on PATH; render.available_docx is checked before the format is usable.
const FORMATS = [
  { value: "pdf", label: "PDF", extension: "pdf" },
  { value: "docx", label: "DOCX", extension: "docx" },
];

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
  const [docxAvailable, setDocxAvailable] = useState<boolean | null>(null);
  const extension =
    FORMATS.find((candidate) => candidate.value === format)?.extension ?? "pdf";

  useEffect(() => {
    if (!open) return;
    renderAvailableDocx().then(setDocxAvailable, () => setDocxAvailable(false));
  }, [open]);

  const docxUnavailable = format === "docx" && docxAvailable === false;

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
            {docxUnavailable && (
              <p className="text-xs text-muted-foreground">
                pandoc is not on PATH, so DOCX cannot be exported.
              </p>
            )}
          </div>
        </div>

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button
            disabled={busy || name.trim() === "" || docxUnavailable}
            onClick={() => {
              setBusy(true);
              const rendered = format === "docx" ? renderPreviewDocx() : renderPreview();
              rendered.then(
                () => {
                  setBusy(false);
                  onOpenChange(false);
                  // Refreshes the on-screen PDF preview; DOCX has nothing on screen to refresh.
                  if (format === "pdf") onRender();
                  window.location.href = `/preview.${extension}?download=${encodeURIComponent(
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
