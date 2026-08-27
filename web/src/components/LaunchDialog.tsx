import { useMemo, useState } from "react";
import { FilePlus2, FolderOpen, GitBranch, Loader2, PackagePlus } from "lucide-react";

import { BLANK_APPLICATION } from "@/components/ApplicationTracker";
import { arrange } from "@/components/ResumeTree";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { go } from "@/lib/route";
import {
  branchName,
  checkoutBranch,
  createApplication,
  createBranch,
  type Branch,
} from "@/rpc";

type View = "menu" | "new-resume" | "open-existing";

function Tile({
  icon,
  title,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-3 rounded-lg border p-3 text-left transition-colors hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span className="text-muted-foreground">{icon}</span>
      <span className="text-sm font-medium">{title}</span>
    </button>
  );
}

export default function LaunchDialog({
  open,
  onOpenChange,
  branches,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  branches: Branch[];
  onChanged: () => void;
}) {
  const [view, setView] = useState<View>("menu");
  const [label, setLabel] = useState("");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setView("menu");
    setLabel("");
    setQuery("");
    setError(null);
  };

  const close = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  const openBranch = (name: string) => {
    setBusy(true);
    setError(null);
    checkoutBranch(name).then(
      () => {
        setBusy(false);
        close(false);
        go("editor");
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const createResume = () => {
    setBusy(true);
    setError(null);
    branchName(label, null)
      .then((name) => createBranch(name, trunk?.name ?? "main").then(() => name))
      .then(openBranch)
      .catch((failure: Error) => {
        setBusy(false);
        setError(failure.message);
      });
  };

  const createApplicationDraft = () => {
    setBusy(true);
    setError(null);
    createApplication(BLANK_APPLICATION).then(
      () => {
        setBusy(false);
        close(false);
        onChanged();
      },
      (failure: Error) => {
        setBusy(false);
        setError(failure.message);
      },
    );
  };

  const { trunk, roots, loose } = useMemo(() => arrange(branches), [branches]);
  const flattened = useMemo(() => {
    const all: Branch[] = [];
    if (trunk) all.push(trunk);
    for (const node of roots) {
      all.push(node.branch);
      for (const child of node.children) all.push(child.branch);
    }
    all.push(...loose);
    return all;
  }, [trunk, roots, loose]);
  const filtered = flattened.filter((branch) =>
    branch.name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {view === "menu" && "New"}
            {view === "new-resume" && "New resume"}
            {view === "open-existing" && "Open a resume"}
          </DialogTitle>
        </DialogHeader>

        {view === "menu" && (
          <div className="grid gap-2">
            <Tile
              icon={<GitBranch className="size-4" />}
              title="New resume"
              onClick={() => setView("new-resume")}
            />
            <Tile
              icon={<PackagePlus className="size-4" />}
              title="New application"
              onClick={createApplicationDraft}
            />
            <Tile
              icon={<FolderOpen className="size-4" />}
              title="Open existing"
              onClick={() => setView("open-existing")}
            />
          </div>
        )}

        {view === "new-resume" && (
          <div className="grid gap-3">
            <Input
              autoFocus
              value={label}
              placeholder="Systems programmer"
              onChange={(event) => setLabel(event.target.value)}
              onKeyDown={(event) => event.key === "Enter" && label.trim() && createResume()}
            />
            <div className="flex justify-between">
              <Button variant="ghost" size="sm" onClick={() => setView("menu")}>
                Back
              </Button>
              <Button size="sm" disabled={busy || !label.trim()} onClick={createResume}>
                {busy ? <Loader2 className="size-3.5 animate-spin" /> : <FilePlus2 className="size-3.5" />}
                Create
              </Button>
            </div>
          </div>
        )}

        {view === "open-existing" && (
          <div className="grid gap-3">
            <Input
              autoFocus
              value={query}
              placeholder="Filter by name"
              onChange={(event) => setQuery(event.target.value)}
            />
            <div className="max-h-72 overflow-auto rounded-md border">
              {filtered.length === 0 ? (
                <p className="p-3 text-center text-xs text-muted-foreground">No branches match.</p>
              ) : (
                filtered.map((branch) => (
                  <button
                    key={branch.name}
                    type="button"
                    disabled={busy}
                    onClick={() => openBranch(branch.name)}
                    className="flex w-full items-center gap-2 border-b px-3 py-2 text-left text-sm last:border-b-0 hover:bg-muted/60 disabled:opacity-50"
                  >
                    <GitBranch className="size-3.5 shrink-0 text-muted-foreground" />
                    <span className="truncate">{branch.name}</span>
                  </button>
                ))
              )}
            </div>
            <Button variant="ghost" size="sm" className="justify-self-start" onClick={() => setView("menu")}>
              Back
            </Button>
          </div>
        )}

        {error && (
          <p className="text-xs text-destructive" role="alert">
            {error}
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
