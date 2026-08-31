import { useEffect, useRef, useState } from "react";

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
  beginGoogleAuth,
  disconnectGoogle,
  googleStatus,
  setGoogleCredentials,
  type GoogleStatus,
} from "@/rpc";

// How long to keep checking for the OAuth redirect to complete, once the consent tab is opened.
// The only polling in this codebase — everywhere else refetches once, on mount or after an
// action — justified here because completing the flow happens in a browser tab this page cannot
// otherwise observe.
const POLL_INTERVAL_MS = 1500;
const POLL_TIMEOUT_MS = 2 * 60 * 1000;

export default function GoogleAccountDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [status, setStatus] = useState<GoogleStatus | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pollTimer = useRef<number | null>(null);

  const stopPolling = () => {
    if (pollTimer.current !== null) {
      window.clearInterval(pollTimer.current);
      pollTimer.current = null;
    }
  };

  useEffect(() => {
    if (!open) return;
    setClientId("");
    setClientSecret("");
    setError(null);
    googleStatus().then(setStatus, (failure: Error) => setError(failure.message));
    return stopPolling;
  }, [open]);

  const saveCredentials = () => {
    setBusy(true);
    setError(null);
    setGoogleCredentials(clientId.trim() || null, clientSecret.trim() || null).then(
      () => {
        setBusy(false);
        setClientId("");
        setClientSecret("");
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const connect = () => {
    setError(null);
    beginGoogleAuth().then((url) => {
      window.open(url, "_blank");
      const startedAt = Date.now();
      pollTimer.current = window.setInterval(() => {
        if (Date.now() - startedAt > POLL_TIMEOUT_MS) {
          stopPolling();
          return;
        }
        googleStatus().then((next) => {
          setStatus(next);
          if (next.connected) stopPolling();
        });
      }, POLL_INTERVAL_MS);
    }, (failure: Error) => setError(failure.message));
  };

  const disconnect = () => {
    setBusy(true);
    setError(null);
    disconnectGoogle().then(
      () => {
        setBusy(false);
        setStatus({ connected: false, account_email: null });
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
          <DialogTitle>Google Sheets sync</DialogTitle>
        </DialogHeader>

        <div className="grid gap-3">
          <p className="text-xs text-muted-foreground">
            {status === null
              ? "Checking…"
              : status.connected
                ? `Connected${status.account_email ? ` as ${status.account_email}` : ""}.`
                : "Not connected."}
          </p>

          <div className="grid gap-1.5">
            <Label htmlFor="google-client-id">Google OAuth client id</Label>
            <Input
              id="google-client-id"
              value={clientId}
              onChange={(event) => setClientId(event.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="google-client-secret">Google OAuth client secret</Label>
            <Input
              id="google-client-secret"
              type="password"
              value={clientSecret}
              onChange={(event) => setClientSecret(event.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
          </div>
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          {status?.connected && (
            <Button variant="ghost" disabled={busy} onClick={disconnect}>
              Disconnect
            </Button>
          )}
          <DialogClose render={<Button variant="ghost">Close</Button>} />
          <Button
            variant="secondary"
            disabled={busy || (clientId.trim() === "" && clientSecret.trim() === "")}
            onClick={saveCredentials}
          >
            Save
          </Button>
          <Button disabled={busy} onClick={connect}>
            Connect Google Account
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
