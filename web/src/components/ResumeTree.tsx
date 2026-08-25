import { useCallback, useEffect, useState } from "react";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Check,
  GitBranch,
  Loader2,
  Pencil,
  Plus,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { go } from "@/lib/route";
import { cn } from "@/lib/utils";
import {
  branchName,
  checkoutBranch,
  createBranch,
  mergeBranch,
  repositoryState,
  type Application,
  type Branch,
} from "@/rpc";

const TRUNK = ["main", "master"];

interface Node {
  branch: Branch;
  children: Node[];
}

/// Groups branches by the hierarchy their names carry: trunk, archetypes, then the resumes
/// tailored from each. A branch following no convention is listed on its own.
function arrange(branches: Branch[]): { trunk: Branch | null; roots: Node[]; loose: Branch[] } {
  const trunk = branches.find((branch) => TRUNK.includes(branch.name)) ?? null;
  const archetypes = new Map<string, Node>();
  const loose: Branch[] = [];
  const orphans: Branch[] = [];

  for (const branch of branches) {
    if (branch === trunk) continue;
    const archetype = branch.name.match(/^archetype\/([^/]+)$/);
    if (archetype) {
      archetypes.set(archetype[1], { branch, children: [] });
      continue;
    }
    if (/^apply\/[^/]+\/[^/]+$/.test(branch.name)) {
      orphans.push(branch);
      continue;
    }
    loose.push(branch);
  }

  for (const branch of orphans) {
    const slug = branch.name.split("/")[1];
    const parent = archetypes.get(slug);
    if (parent) parent.children.push({ branch, children: [] });
    else loose.push(branch);
  }

  return { trunk, roots: [...archetypes.values()], loose };
}

function Row({
  branch,
  depth,
  applications,
  onCheckout,
  onMerge,
  onAdd,
  busy,
}: {
  branch: Branch;
  depth: number;
  applications: Application[];
  onCheckout: (name: string) => void;
  onMerge: (from: string, into: string, direction: "up" | "down") => void;
  onAdd?: () => void;
  busy: string | null;
  }) {
  const linked = applications.filter((one) => one.resume_branch === branch.name);
  const settled = linked.some((one) => one.status === "offer" || one.status === "rejected");
  const parent =
    branch.name.startsWith("apply/")
      ? `archetype/${branch.name.split("/")[1]}`
      : branch.name.startsWith("archetype/")
        ? "main"
        : null;

  return (
    <div
      className={cn(
        "group flex items-center gap-2 rounded-md px-2 py-2 hover:bg-muted/50",
        branch.is_head && "bg-muted/70",
      )}
      style={{ paddingLeft: `${depth * 1.5 + 0.5}rem` }}
    >
      <GitBranch
        className={cn(
          "size-3.5 shrink-0",
          branch.is_head ? "text-foreground" : "text-muted-foreground/60",
        )}
      />
      <button
        type="button"
        className="min-w-0 flex-1 text-left"
        onClick={() => onCheckout(branch.name)}
      >
        <span className="flex items-center gap-2">
          <span className="truncate text-sm font-medium">
            {branch.name.split("/").pop()}
          </span>
          {branch.is_head && (
            <span className="flex items-center gap-1 rounded-full bg-foreground px-1.5 py-0.5 text-[10px] font-medium text-background">
              <Check className="size-2.5" />
              open
            </span>
          )}
          {settled && (
            <span className="rounded-full border px-1.5 py-0.5 text-[10px] text-muted-foreground">
              settled
            </span>
          )}
        </span>
        <span className="mt-0.5 block truncate font-mono text-[10px] text-muted-foreground">
          {branch.name}
          {linked.length > 0 &&
            ` · ${linked.length} application${linked.length === 1 ? "" : "s"}`}
        </span>
      </button>

      <div className="flex shrink-0 items-center gap-0.5 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
        {onAdd && (
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            title="Tailor a resume from this archetype"
            onClick={onAdd}
          >
            <Plus className="size-3.5" />
          </Button>
        )}
        {parent && (
          <>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              title={`Send this wording up into ${parent}`}
              disabled={busy !== null}
              onClick={() => onMerge(branch.name, parent, "up")}
            >
              {busy === `up:${branch.name}` ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <ArrowUpFromLine className="size-3.5" />
              )}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              title={`Take ${parent}'s changes into this one`}
              disabled={busy !== null}
              onClick={() => onMerge(parent, branch.name, "down")}
            >
              {busy === `down:${branch.name}` ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <ArrowDownToLine className="size-3.5" />
              )}
            </Button>
          </>
        )}
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          title="Open in the editor"
          onClick={() => {
            onCheckout(branch.name);
            go("editor");
          }}
        >
          <Pencil className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

export default function ResumeTree({
  applications,
  onChanged,
}: {
  applications: Application[];
  onChanged: () => void;
}) {
  const [branches, setBranches] = useState<Branch[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [adding, setAdding] = useState<string | null>(null);
  const [label, setLabel] = useState("");

  const load = useCallback(() => {
    repositoryState().then(
      (state) => setBranches(state.branches),
      (failure: Error) => setError(failure.message),
    );
  }, []);

  useEffect(load, [load]);

  const refresh = () => {
    load();
    onChanged();
  };

  const checkout = (name: string) => {
    setError(null);
    checkoutBranch(name).then(refresh, (failure: Error) => setError(failure.message));
  };

  const merge = (from: string, into: string, direction: "up" | "down") => {
    const marker = `${direction}:${direction === "up" ? from : into}`;
    setBusy(marker);
    setError(null);
    setNotice(null);
    checkoutBranch(into)
      .then(() => mergeBranch(from))
      .then((outcome) => {
        setBusy(null);
        setNotice(
          outcome.kind === "already-current"
            ? `${into} already has everything from ${from}.`
            : `${from} → ${into}: ${outcome.kind}.`,
        );
        refresh();
      })
      .catch((failure: Error) => {
        setBusy(null);
        setError(failure.message);
        refresh();
      });
  };

  const addChild = (parent: string) => {
    const slug = parent.replace(/^archetype\//, "");
    branchName(slug, label)
      .then((name) => createBranch(name, parent).then(() => checkoutBranch(name)))
      .then(() => {
        setAdding(null);
        setLabel("");
        refresh();
      })
      .catch((failure: Error) => setError(failure.message));
  };

  const addArchetype = () => {
    branchName(label, null)
      .then((name) => createBranch(name, "main").then(() => checkoutBranch(name)))
      .then(() => {
        setAdding(null);
        setLabel("");
        refresh();
      })
      .catch((failure: Error) => setError(failure.message));
  };

  if (error && !branches) {
    return <p className="rounded-md border p-4 text-sm text-destructive">{error}</p>;
  }
  if (!branches) {
    return <p className="p-4 text-sm text-muted-foreground">Reading the repository…</p>;
  }

  const { trunk, roots, loose } = arrange(branches);

  return (
    <section className="rounded-lg border bg-card">
      <header className="flex items-center gap-2 border-b px-4 py-3">
        <h2 className="text-sm font-semibold">Resumes</h2>
        <span className="flex-1" />
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1.5 text-xs"
          onClick={() => {
            setAdding(adding === "__archetype__" ? null : "__archetype__");
            setLabel("");
          }}
        >
          <Plus className="size-3.5" />
          New archetype
        </Button>
      </header>

      {adding === "__archetype__" && (
        <div className="flex items-center gap-2 border-b bg-muted/40 px-4 py-2">
          <Input
            autoFocus
            value={label}
            placeholder="Systems programmer"
            className="h-7 text-xs"
            onChange={(event) => setLabel(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && label.trim() && addArchetype()}
          />
          <Button size="sm" className="h-7 text-xs" disabled={!label.trim()} onClick={addArchetype}>
            Create
          </Button>
        </div>
      )}

      <div className="p-2">
        {trunk && (
          <Row
            branch={trunk}
            depth={0}
            applications={applications}
            onCheckout={checkout}
            onMerge={merge}
            busy={busy}
          />
        )}

        {roots.map((node) => (
          <div key={node.branch.name}>
            <Row
              branch={node.branch}
              depth={1}
              applications={applications}
              onCheckout={checkout}
              onMerge={merge}
              busy={busy}
              onAdd={() => {
                setAdding(node.branch.name);
                setLabel("");
              }}
            />
            {adding === node.branch.name && (
              <div className="flex items-center gap-2 py-1 pl-12">
                <Input
                  autoFocus
                  value={label}
                  placeholder="Acme Corp"
                  className="h-7 text-xs"
                  onChange={(event) => setLabel(event.target.value)}
                  onKeyDown={(event) =>
                    event.key === "Enter" && label.trim() && addChild(node.branch.name)
                  }
                />
                <Button
                  size="sm"
                  className="h-7 text-xs"
                  disabled={!label.trim()}
                  onClick={() => addChild(node.branch.name)}
                >
                  Create
                </Button>
              </div>
            )}
            {node.children.map((child) => (
              <Row
                key={child.branch.name}
                branch={child.branch}
                depth={2}
                applications={applications}
                onCheckout={checkout}
                onMerge={merge}
                busy={busy}
              />
            ))}
          </div>
        ))}

        {roots.length === 0 && (
          <p className="px-2 py-6 text-center text-sm text-muted-foreground">
            No archetypes yet. Start one for a kind of role — systems programmer,
            frontend, research — then tailor resumes from it.
          </p>
        )}

        {loose.length > 0 && (
          <>
            <p className="mt-3 px-2 pb-1 text-[10px] tracking-wide text-muted-foreground uppercase">
              Other branches
            </p>
            {loose.map((branch) => (
              <Row
                key={branch.name}
                branch={branch}
                depth={0}
                applications={applications}
                onCheckout={checkout}
                onMerge={merge}
                busy={busy}
              />
            ))}
          </>
        )}
      </div>

      {(notice || error) && (
        <p
          className={cn(
            "border-t px-4 py-2 font-mono text-xs whitespace-pre-wrap",
            error ? "text-destructive" : "text-muted-foreground",
          )}
        >
          {error ?? notice}
        </p>
      )}
    </section>
  );
}
