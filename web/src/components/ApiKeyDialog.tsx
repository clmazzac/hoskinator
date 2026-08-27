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
import { aiStatus, setAnthropicApiKey } from "@/rpc";

export default function ApiKeyDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setKey("");
    setError(null);
    aiStatus().then(setConfigured, (failure: Error) => setError(failure.message));
  }, [open]);

  const apply = (next: string | null) => {
    setBusy(true);
    setError(null);
    setAnthropicApiKey(next).then(
      (result) => {
        setBusy(false);
        setConfigured(result);
        setKey("");
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
          <DialogTitle>Anthropic API key</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <p className="text-xs text-muted-foreground">
            {configured === null
              ? "Checking…"
              : configured
                ? "A key is configured."
                : "No key is configured."}
          </p>
          <div className="grid gap-1.5">
            <Label htmlFor="anthropic-key">New key</Label>
            <Input
              id="anthropic-key"
              type="password"
              value={key}
              onChange={(event) => setKey(event.target.value)}
              placeholder="sk-ant-…"
              spellCheck={false}
              autoComplete="off"
            />
          </div>
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          {configured && (
            <Button variant="ghost" disabled={busy} onClick={() => apply(null)}>
              Remove
            </Button>
          )}
          <DialogClose render={<Button variant="ghost">Close</Button>} />
          <Button disabled={busy || key.trim() === ""} onClick={() => apply(key.trim())}>
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
