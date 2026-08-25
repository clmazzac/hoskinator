import { useCallback, useEffect, useState } from "react";
import { FolderGit2, Loader2, PanelsTopLeft, UploadCloud } from "lucide-react";

import ApplicationTracker from "@/components/ApplicationTracker";
import RepositorySetup from "@/components/RepositorySetup";
import ResumeTree from "@/components/ResumeTree";
import { Button } from "@/components/ui/button";
import { go } from "@/lib/route";
import { isDark, setDark } from "@/lib/theme";
import {
  commitResume,
  listApplications,
  pushBranch,
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
  const [head, setHead] = useState<string | null>(null);
  const [dirty, setDirty] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [dark, setDarkState] = useState(isDark);

  const load = useCallback(() => {
    workspaceStatus().then(setStatus, () => setStatus(null));
    listApplications().then(setApplications, () => setApplications([]));
    repositoryState().then(
      (state) => {
        setBranches(state.branches);
        setHead(state.head?.branch ?? null);
      },
      () => setBranches([]),
    );
    repositoryStatus().then(
      (state) => setDirty(state.entries.length),
      () => setDirty(0),
    );
  }, []);

  useEffect(load, [load]);

  const saveAll = () => {
    setBusy(true);
    setNotice(null);
    commitResume("Update resume")
      .then(() => (head ? pushBranch(head) : Promise.resolve(null)))
      .then(() => {
        setBusy(false);
        setNotice("Committed and pushed.");
        load();
      })
      .catch((failure: Error) => {
        setBusy(false);
        setNotice(failure.message);
        load();
      });
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
          {status.github_login && (
            <span className="hidden items-center gap-1.5 text-xs text-muted-foreground sm:flex">
              <FolderGit2 className="size-3.5" />
              {status.github_login}
              {status.remote_url && <span className="text-muted-foreground/50">·</span>}
              {status.remote_url && (
                <span className="max-w-56 truncate font-mono text-[10px]">
                  {status.remote_url.replace(/^.*[:/]([^/]+\/[^/]+?)(\.git)?$/, "$1")}
                </span>
              )}
            </span>
          )}

          <span className="flex-1" />

          {ready && (
            <>
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
                  <UploadCloud className="size-3.5" />
                )}
                Save &amp; push
              </Button>
              <Button
                size="sm"
                className="h-7 gap-1.5 text-xs"
                onClick={() => go("editor")}
              >
                <PanelsTopLeft className="size-3.5" />
                Open editor
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={() => {
              const next = !dark;
              setDark(next);
              setDarkState(next);
            }}
          >
            {dark ? "Day" : "Night"}
          </Button>
        </div>
      </header>

      <div className="mx-auto max-w-6xl px-6 py-8">
        {!ready ? (
          <>
            <div className="mx-auto mb-8 max-w-3xl">
              <h2 className="text-lg font-semibold">Keep every resume in one repository</h2>
              <p className="mt-1 text-sm text-muted-foreground">
                One branch per resume. Start an archetype for a kind of role, tailor a
                copy for each application, and move wording between them.
              </p>
            </div>
            <RepositorySetup status={status} onReady={setStatus} />
          </>
        ) : (
          <div className="grid gap-6">
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
    </main>
  );
}
