import { useEffect, useState } from "react";
import { Check, FolderGit2, Loader2, Plus, Link2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  connectRepository,
  createGithubRepository,
  ownedRepositories,
  type WorkspaceStatus,
} from "@/rpc";

/// Where a new repository is cloned, unless the user says otherwise.
function suggestedPath(name: string): string {
  const home = "/home/cam";
  return `${home}/${name || "resumes"}`;
}

export default function RepositorySetup({
  status,
  onReady,
}: {
  status: WorkspaceStatus;
  onReady: (status: WorkspaceStatus) => void;
}) {
  const [name, setName] = useState("resumes");
  const [source, setSource] = useState("");
  const [owned, setOwned] = useState<string[] | null>(null);
  const [busy, setBusy] = useState<"create" | "connect" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!status.github_login) return;
    ownedRepositories().then(setOwned, () => setOwned([]));
  }, [status.github_login]);

  const attempt = (which: "create" | "connect", work: Promise<WorkspaceStatus>) => {
    setBusy(which);
    setError(null);
    work.then(
      (next) => {
        setBusy(null);
        onReady(next);
      },
      (failure: Error) => {
        setBusy(null);
        setError(failure.message);
      },
    );
  };

  if (!status.gh_installed) {
    return (
      <Card title="GitHub CLI required">
        <p className="text-sm text-muted-foreground">
          Hoskinator reaches GitHub through the <code className="font-mono">gh</code>{" "}
          command, so your token never passes through this process. Install it, then
          reload.
        </p>
        <pre className="mt-3 rounded-md bg-muted p-3 font-mono text-xs">
          brew install gh && gh auth login
        </pre>
      </Card>
    );
  }

  if (!status.github_login) {
    return (
      <Card title="Sign in to GitHub">
        <p className="text-sm text-muted-foreground">
          Run this in a terminal, then reload. Hoskinator never sees the token.
        </p>
        <pre className="mt-3 rounded-md bg-muted p-3 font-mono text-xs">
          gh auth login
        </pre>
      </Card>
    );
  }

  return (
    <div className="mx-auto grid w-full max-w-3xl gap-4">
      <header className="flex items-center gap-2 text-sm text-muted-foreground">
        <FolderGit2 className="size-4" />
        Signed in as
        <span className="font-medium text-foreground">{status.github_login}</span>
        <Check className="size-3.5" />
      </header>

      <div className="grid gap-4 sm:grid-cols-2">
        <Card title="Start a new repository">
          <p className="text-sm text-muted-foreground">
            Creates a private repository on your account and clones it.
          </p>
          <div className="mt-4 grid gap-2">
            <Label htmlFor="repo-name" className="text-xs">
              Repository name
            </Label>
            <Input
              id="repo-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              spellCheck={false}
            />
            <p className="font-mono text-[11px] text-muted-foreground">
              {suggestedPath(name)}
            </p>
          </div>
          <Button
            className="mt-4 w-full gap-1.5"
            disabled={busy !== null || name.trim() === ""}
            onClick={() =>
              attempt(
                "create",
                createGithubRepository(name.trim(), suggestedPath(name.trim())),
              )
            }
          >
            {busy === "create" ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Plus className="size-3.5" />
            )}
            Create private repository
          </Button>
        </Card>

        <Card title="Connect an existing one">
          <p className="text-sm text-muted-foreground">
            Pick a repository you already keep resumes in.
          </p>
          <div className="mt-4 grid gap-2">
            <Label htmlFor="repo-source" className="text-xs">
              Repository
            </Label>
            {owned && owned.length > 0 ? (
              <Select value={source} onValueChange={(next) => next && setSource(next)}>
                <SelectTrigger id="repo-source">
                  <SelectValue placeholder="Choose a repository" />
                </SelectTrigger>
                <SelectContent>
                  {owned.map((repository) => (
                    <SelectItem key={repository} value={repository}>
                      {repository}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <Input
                id="repo-source"
                value={source}
                onChange={(event) => setSource(event.target.value)}
                placeholder="owner/name"
                spellCheck={false}
              />
            )}
            <p className="font-mono text-[11px] text-muted-foreground">
              {suggestedPath(source.split("/").pop() ?? "")}
            </p>
          </div>
          <Button
            variant="outline"
            className="mt-4 w-full gap-1.5"
            disabled={busy !== null || source.trim() === ""}
            onClick={() =>
              attempt(
                "connect",
                connectRepository(
                  source.trim(),
                  suggestedPath(source.trim().split("/").pop() ?? "resumes"),
                ),
              )
            }
          >
            {busy === "connect" ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Link2 className="size-3.5" />
            )}
            Connect
          </Button>
        </Card>
      </div>

      {error && (
        <p className="rounded-md border border-destructive/30 bg-destructive/5 p-3 font-mono text-xs whitespace-pre-wrap text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-lg border bg-card p-5">
      <h2 className="text-sm font-semibold">{title}</h2>
      <div className="mt-2">{children}</div>
    </section>
  );
}
