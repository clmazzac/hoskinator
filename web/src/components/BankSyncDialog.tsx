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
import { bankPull, bankPush, bankSetCredentials, bankStatus, type BankStatus } from "@/rpc";

export default function BankSyncDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [status, setStatus] = useState<BankStatus | null>(null);
  const [url, setUrl] = useState("");
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setUrl("");
    setToken("");
    setNotice(null);
    setError(null);
    bankStatus().then(setStatus, (failure: Error) => setError(failure.message));
  }, [open]);

  const save = () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    bankSetCredentials(url.trim() || null, token.trim() || null).then(
      () => {
        setBusy(false);
        setUrl("");
        setToken("");
        bankStatus().then(setStatus, (failure: Error) => setError(failure.message));
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const push = () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    bankPush().then(
      () => {
        setBusy(false);
        setNotice("Pushed. Turso now matches the local bank.");
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const pull = () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    bankPull().then(
      (pulled) => {
        setBusy(false);
        setNotice(
          pulled
            ? "Pulled. The local bank now matches Turso."
            : "Nothing has been pushed to Turso yet.",
        );
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const configured = status?.configured ?? false;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Bank sync</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <p className="text-xs text-muted-foreground">
            {status === null
              ? "Checking…"
              : configured
                ? `Syncing against ${status.url}.`
                : "No Turso database is configured."}
          </p>
          <div className="grid gap-1.5">
            <Label htmlFor="turso-url">Database URL</Label>
            <Input
              id="turso-url"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="libsql://<database>.turso.io"
              spellCheck={false}
              autoComplete="off"
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="turso-token">Auth token</Label>
            <Input
              id="turso-token"
              type="password"
              value={token}
              onChange={(event) => setToken(event.target.value)}
              placeholder="…"
              spellCheck={false}
              autoComplete="off"
            />
          </div>
          <p className="text-xs text-muted-foreground">
            Push replaces Turso's copy with the local bank. Pull replaces the local bank with
            Turso's copy. Neither runs on its own.
          </p>
          {notice && <p className="text-xs text-muted-foreground">{notice}</p>}
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Close</Button>} />
          <Button variant="outline" disabled={busy || !configured} onClick={pull}>
            Pull
          </Button>
          <Button variant="outline" disabled={busy || !configured} onClick={push}>
            Push
          </Button>
          <Button disabled={busy || url.trim() === "" || token.trim() === ""} onClick={save}>
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
