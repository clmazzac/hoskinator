import { useCallback, useEffect, useState } from "react";
import { FolderGit2, Loader2, Moon, Plus, Sun, UploadCloud } from "lucide-react";

import ApplicationTracker from "@/components/ApplicationTracker";
import GithubDialog from "@/components/GithubDialog";
import LaunchDialog from "@/components/LaunchDialog";
import RepositorySetup from "@/components/RepositorySetup";
import ResumeTree from "@/components/ResumeTree";
import { Button } from "@/components/ui/button";
import { isDark, setDark } from "@/lib/theme";
import { toCsv } from "@/lib/sheet";
import {
  commitResume,
  githubStatus,
  listApplications,
  pushBranch,
  repositoryState,
  repositoryStatus,
  workspaceStatus,
  writeStagedFile,
  type Application,
  type Branch,
  type GithubStatus,
  type WorkspaceStatus,
} from "@/rpc";

/// Where the application tracker rides along in the repository, so it travels with git rather
/// than living only in the local store.
const APPLICATIONS_FILE = "applications.csv";

export default function Home() {
  const [status, setStatus] = useState<WorkspaceStatus | null>(null);
  const [applications, setApplications] = useState<Application[]>([]);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [head, setHead] = useState<string | null>(null);
  const [dirty, setDirty] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [dark, setDarkState] = useState(isDark);
  const [launching, setLaunching] = useState(false);
  const [github, setGithub] = useState<GithubStatus | null>(null);
  const [connecting, setConnecting] = useState(false);

  const load = useCallback(() => {
    workspaceStatus().then(setStatus, () => setStatus(null));
    githubStatus().then(setGithub, () => setGithub(null));
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
    // Mirrors the tracker into the repo alongside resume.yaml. Best-effort: a failure here
    // (e.g. no repository yet) must not stop the resume itself from saving.
    writeStagedFile(APPLICATIONS_FILE, toCsv(applications))
      .catch(() => null)
      // The commit and the push fail for different reasons — a repository with no remote commits
      // perfectly well — so a failed push must not read as a failed commit.
      .then(() => commitResume("Update resume"))
      .then(
        () =>
          (head ? pushBranch(head) : Promise.resolve(null)).then(
            () => "Committed and pushed.",
            (failure: Error) => `Committed. Push failed: ${failure.message}`,
          ),
        (failure: Error) => `Nothing committed: ${failure.message}`,
      )
      .then((said) => {
        setBusy(false);
        setNotice(said);
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

          {github?.connected && (
            <button
              type="button"
              className="hidden items-center gap-1.5 text-xs text-muted-foreground sm:flex"
              title="GitHub connection — click to manage"
              onClick={() => setConnecting(true)}
            >
              <FolderGit2 className="size-3.5" />
              {github.login}
            </button>
          )}
          {!github?.connected && (
            <Button
              variant="outline"
              size="sm"
              className="h-7 gap-1.5 text-xs"
              onClick={() => setConnecting(true)}
            >
              Connect GitHub
            </Button>
          )}

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
                onClick={() => setLaunching(true)}
              >
                <Plus className="size-3.5" />
                New
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="h-7 gap-1.5 text-xs"
            onClick={() => {
              const next = !dark;
              setDark(next);
              setDarkState(next);
            }}
          >
            {dark ? <Sun className="size-3.5" /> : <Moon className="size-3.5" />}
            {dark ? "Day" : "Night"}
          </Button>
        </div>
      </header>

      {connecting && (
        <GithubDialog status={github} onOpenChange={setConnecting} onChanged={load} />
      )}

      <div className="mx-auto max-w-6xl px-6 py-8">
        {!ready ? (
          <>
            <div className="mx-auto mb-8 max-w-3xl">
              <h2 className="text-lg font-semibold">Keep every resume in one repository</h2>
              <p className="mt-1 text-sm text-muted-foreground">
                One branch per resume. Branch for a kind of role, tailor a copy for
                each application, and move wording between them.
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

      <LaunchDialog
        open={launching}
        onOpenChange={setLaunching}
        branches={branches}
        onChanged={load}
      />
    </main>
  );
}
