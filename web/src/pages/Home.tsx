import { useCallback, useEffect, useState } from "react";
import { FolderGit2, Loader2, Moon, Plus, Save, Sun } from "lucide-react";

import ApplicationTracker from "@/components/ApplicationTracker";
import LaunchDialog from "@/components/LaunchDialog";
import RepositoryDialog from "@/components/RepositoryDialog";
import RepositorySetup from "@/components/RepositorySetup";
import ResumeTree from "@/components/ResumeTree";
import { Button } from "@/components/ui/button";
import { isDark, setDark } from "@/lib/theme";
import { repositorySlug } from "@/lib/utils";
import {
  commitResume,
  listApplications,
  repositoryState,
  repositoryStatus,
  workspaceStatus,
  type Application,
  type Branch,
  type WorkspaceStatus,
} from "@/rpc";

export default function Home() {
  const [status, setStatus] = useState<WorkspaceStatus | null>(null);
  const [applications, setApplications] = useState<Application[]>([]);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [dirty, setDirty] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [dark, setDarkState] = useState(isDark);
  const [launching, setLaunching] = useState(false);
  const [managingRepository, setManagingRepository] = useState(false);

  const load = useCallback(() => {
    workspaceStatus().then(setStatus, () => setStatus(null));
    listApplications().then(setApplications, () => setApplications([]));
    repositoryState().then(
      (state) => setBranches(state.branches),
      () => setBranches([]),
    );
    repositoryStatus().then(
      (state) => setDirty(state.entries.length),
      () => setDirty(0),
    );
  }, []);

  useEffect(load, [load]);

  // Commits to the local repository only. Pushing to `origin` is the user's own `git push`.
  const saveAll = () => {
    setBusy(true);
    setNotice(null);
    commitResume("Update resume").then(
      () => {
        setBusy(false);
        setNotice("Saved.");
        load();
      },
      (failure: Error) => {
        setBusy(false);
        setNotice(`Nothing to save: ${failure.message}`);
        load();
      },
    );
  };

  if (!status) {
    return (
      <main className="grid min-h-dvh place-items-center bg-background text-foreground">
        <Loader2 className="size-5 animate-spin text-muted-foreground" />
      </main>
    );
  }

  const ready = status.repository_ready;

  return (
    <main className="min-h-dvh bg-background text-foreground">
      <header className="sticky top-0 z-10 border-b bg-background/80 backdrop-blur">
        <div className="mx-auto flex max-w-6xl items-center gap-3 px-6 py-3">
          <h1 className="text-sm font-semibold tracking-tight">Hoskinator</h1>

          <span className="flex-1" />

          {ready && (
            <>
              <Button
                variant="outline"
                size="sm"
                className="h-7 gap-1.5 text-xs"
                title="Which repository this is, and how to switch"
                onClick={() => setManagingRepository(true)}
              >
                <FolderGit2 className="size-3.5" />
                <span className="max-w-40 truncate font-mono">
                  {repositorySlug(status.remote_url) ?? "Repository"}
                </span>
              </Button>

              {dirty > 0 && (
                <span className="text-xs text-muted-foreground tabular-nums">
                  {dirty} unsaved change{dirty === 1 ? "" : "s"}
                </span>
              )}
              <Button
                variant="outline"
                size="sm"
                className="h-7 gap-1.5 text-xs"
                disabled={busy || dirty === 0}
                onClick={saveAll}
              >
                {busy ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Save className="size-3.5" />
                )}
                Save
              </Button>
              <Button
                size="sm"
                className="h-7 gap-1.5 text-xs"
                onClick={() => setLaunching(true)}
              >
                <Plus className="size-3.5" />
                New
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={dark ? "Switch to day theme" : "Switch to night theme"}
            title={dark ? "Switch to day theme" : "Switch to night theme"}
            onClick={() => {
              const next = !dark;
              setDark(next);
              setDarkState(next);
            }}
          >
            {dark ? <Sun className="size-3.5" /> : <Moon className="size-3.5" />}
          </Button>
        </div>
      </header>

      <div className="mx-auto px-6 py-8">
        {!ready ? (
          <RepositorySetup status={status} onReady={setStatus} />
        ) : (
          <div className="grid grid-cols-1 gap-6">
            <ResumeTree applications={applications} onChanged={load} />
            <ApplicationTracker
              applications={applications}
              branches={branches}
              onChanged={load}
            />
          </div>
        )}

        {notice && (
          <p className="mt-4 font-mono text-xs whitespace-pre-wrap text-muted-foreground">
            {notice}
          </p>
        )}
      </div>

      <LaunchDialog
        open={launching}
        onOpenChange={setLaunching}
        branches={branches}
        onChanged={load}
      />

      {ready && (
        <RepositoryDialog
          status={status}
          open={managingRepository}
          onOpenChange={setManagingRepository}
          onReady={setStatus}
        />
      )}
    </main>
  );
}
