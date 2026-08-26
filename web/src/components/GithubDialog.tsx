import { useState } from "react";

import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  authorizeGithub,
  connectGithub,
  deauthorizeGithub,
  githubRepositories,
  type GithubRepository,
  type GithubStatus,
} from "@/rpc";

/// Two steps: authorize with a personal access token, then point the resume repository at a
/// private GitHub repository — typed, or picked from the ones the account already owns.
export default function GithubDialog({
  status,
  onOpenChange,
  onChanged,
}: {
  status: GithubStatus | null;
  onOpenChange: (open: boolean) => void;
  /** Runs whatever closed the dialog needs refreshed — connection state, tree, remote. */
  onChanged: () => void;
}) {
  const [step, setStep] = useState<"token" | "repository">(
    status?.connected ? "repository" : "token",
  );
  const [token, setToken] = useState("");
  const [name, setName] = useState("resume");
  const [repositories, setRepositories] = useState<GithubRepository[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const close = () => {
    onOpenChange(false);
    onChanged();
  };

  const authorize = () => {
    if (!token.trim()) return;
    setBusy(true);
    setError(null);
    authorizeGithub(token.trim())
      .then(() => {
        setToken("");
        setStep("repository");
        githubRepositories()
          .then(setRepositories, (failure: Error) => setError(failure.message));
      })
      .catch((failure: Error) => setError(failure.message))
      .finally(() => setBusy(false));
  };

  const connect = (create: boolean) => {
    if (!name.trim()) return;
    setBusy(true);
    setError(null);
    connectGithub(name.trim(), create)
      .then(() => close())
      .catch((failure: Error) => setError(failure.message))
      .finally(() => setBusy(false));
  };

  const disconnect = () => {
    setBusy(true);
    setError(null);
    deauthorizeGithub()
      .then(close)
      .catch((failure: Error) => setError(failure.message))
      .finally(() => setBusy(false));
  };

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        {error && (
          <p className="text-xs text-destructive" role="alert">
            {error}
          </p>
        )}
        {step === "token" ? (
          <>
            <DialogHeader>
              <DialogTitle>Connect GitHub</DialogTitle>
            </DialogHeader>

            <div className="grid gap-3">
              <p className="text-xs text-muted-foreground">
                Create a personal access token with the{" "}
                <span className="font-mono text-[11px]">repo</span> scope —{" "}
                <a
                  href="https://github.com/settings/tokens/new?scopes=repo&description=Hoskinator"
                  target="_blank"
                  rel="noreferrer"
                  className="underline"
                >
                  github.com/settings/tokens/new
                </a>{" "}
                opens it prefilled. The token stays on this machine.
              </p>
              <div className="grid gap-1.5">
                <Label htmlFor="github-token">Access token</Label>
                <Input
                  id="github-token"
                  autoFocus
                  type="password"
                  value={token}
                  onChange={(event) => setToken(event.target.value)}
                />
              </div>
            </div>

            <DialogFooter>
              <DialogClose render={<Button variant="ghost">Cancel</Button>} />
              <Button disabled={busy || !token.trim()} onClick={authorize}>
                {busy ? "Checking…" : "Authorize"}
              </Button>
            </DialogFooter>
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>Sync to a private repository</DialogTitle>
            </DialogHeader>

            <div className="grid gap-3">
              <p className="text-xs text-muted-foreground">
                Connected as {status?.connected ? status.login : "…"}.
              </p>
              <div className="grid gap-1.5">
                <Label htmlFor="github-repository">Repository</Label>
                <Input
                  id="github-repository"
                  autoFocus
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                />
              </div>
              {repositories !== null && repositories.length > 0 && (
                <div className="max-h-40 overflow-y-auto rounded-md border">
                  {repositories.map((repository) => (
                    <button
                      key={repository.name_with_owner}
                      type="button"
                      className="flex w-full items-center justify-between px-2 py-1.5 text-left text-xs hover:bg-muted/50"
                      onClick={() => setName(repository.name_with_owner)}
                    >
                      <span className="font-mono">{repository.name_with_owner}</span>
                      {repository.private && (
                        <span className="text-[10px] text-muted-foreground">private</span>
                      )}
                    </button>
                  ))}
                </div>
              )}
            </div>

            <DialogFooter>
              <Button
                variant="ghost"
                className="mr-auto"
                disabled={busy}
                onClick={disconnect}
              >
                Disconnect
              </Button>
              <DialogClose render={<Button variant="ghost">Cancel</Button>} />
              <Button variant="outline" disabled={busy || !name.trim()} onClick={() => connect(true)}>
                Create private
              </Button>
              <Button disabled={busy || !name.trim()} onClick={() => connect(false)}>
                Use this one
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
