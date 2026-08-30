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
import { Switch } from "@/components/ui/switch";
import {
  beginGoogleAuth,
  disconnectGoogle,
  googleStatus,
  linkSheet,
  setGoogleCredentials,
  setGoogleSyncEnabled,
  syncGoogleSheetNow,
  workspaceStatus,
  type GoogleStatus,
  type SyncOutcome,
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
  const [syncing, setSyncing] = useState(false);
  const [syncOutcome, setSyncOutcome] = useState<SyncOutcome | null>(null);
  const [linkedSheet, setLinkedSheet] = useState<string | null>(null);
  const [sheetInput, setSheetInput] = useState("");
  const [linking, setLinking] = useState(false);
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
    setSyncOutcome(null);
    setSheetInput("");
    googleStatus().then(setStatus, (failure: Error) => setError(failure.message));
    workspaceStatus().then(
      (status) => setLinkedSheet(status.applications_sheet),
      () => setLinkedSheet(null),
    );
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
        setStatus({
          connected: false,
          account_email: null,
          sync_enabled: false,
          last_synced_at: null,
          last_sync_error: null,
        });
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const link = () => {
    setLinking(true);
    setError(null);
    linkSheet(sheetInput).then(
      (status) => {
        setLinking(false);
        setSheetInput("");
        setLinkedSheet(status.applications_sheet);
      },
      (failure: Error) => {
        setLinking(false);
        setError(failure.message);
      },
    );
  };

  const toggleSync = (enabled: boolean) => {
    setError(null);
    setGoogleSyncEnabled(enabled).then(
      () => setStatus((previous) => previous && { ...previous, sync_enabled: enabled }),
      (failure: Error) => setError(failure.message),
    );
  };

  const sync = () => {
    setSyncing(true);
    setError(null);
    setSyncOutcome(null);
    syncGoogleSheetNow().then(
      (outcome) => {
        setSyncing(false);
        setSyncOutcome(outcome);
      },
      (failure: Error) => {
        setSyncing(false);
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

        <div className="grid grid-cols-1 gap-3">
          <p className="text-xs text-muted-foreground">
            {status === null
              ? "Checking…"
              : status.connected
                ? `Connected${status.account_email ? ` as ${status.account_email}` : ""}.`
                : "Not connected."}
          </p>
          {syncOutcome && (
            <p className="font-mono text-xs text-muted-foreground">
              pulled {syncOutcome.pulled} · pushed {syncOutcome.pushed_cells} cells · created{" "}
              {syncOutcome.created_locally} · appended {syncOutcome.appended_to_sheet}
            </p>
          )}
          {status?.connected && (
            <div className="flex items-center justify-between gap-3">
              <Label htmlFor="google-sync-enabled" className="text-xs font-normal">
                Keep the sheet in sync automatically
              </Label>
              <Switch
                id="google-sync-enabled"
                size="sm"
                checked={status.sync_enabled}
                onCheckedChange={toggleSync}
              />
            </div>
          )}
          {status?.sync_enabled && (
            <p className="font-mono text-xs text-muted-foreground">
              {status.last_sync_error
                ? `last sync failed: ${status.last_sync_error}`
                : status.last_synced_at
                  ? `last synced ${new Date(status.last_synced_at * 1000).toLocaleTimeString()}`
                  : "not synced yet"}
            </p>
          )}

          <div className="grid grid-cols-1 gap-1.5">
            <Label htmlFor="google-sheet-link" className="text-xs">
              Linked sheet
            </Label>
            {linkedSheet && (
              <a
                href={`https://docs.google.com/spreadsheets/d/${linkedSheet}/edit`}
                target="_blank"
                rel="noreferrer"
                className="block truncate font-mono text-xs text-muted-foreground underline underline-offset-2"
              >
                {linkedSheet}
              </a>
            )}
            <div className="flex min-w-0 items-center gap-2">
              <Input
                id="google-sheet-link"
                value={sheetInput}
                onChange={(event) => setSheetInput(event.target.value)}
                placeholder="Paste a Google Sheets link to change it"
                className="h-7 min-w-0 text-xs"
                spellCheck={false}
              />
              <Button
                size="sm"
                variant="outline"
                className="h-7 shrink-0 text-xs"
                disabled={linking || sheetInput.trim() === ""}
                onClick={link}
              >
                {linking ? "Linking…" : "Link"}
              </Button>
            </div>
          </div>

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
            <>
              <Button variant="ghost" disabled={busy} onClick={disconnect}>
                Disconnect
              </Button>
              <Button variant="secondary" disabled={syncing} onClick={sync}>
                {syncing ? "Syncing…" : "Sync now"}
              </Button>
            </>
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
