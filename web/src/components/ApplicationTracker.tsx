import { useEffect, useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronsUpDown,
  ChevronUp,
  Download,
  ExternalLink,
  Plus,
  Sheet as SheetIcon,
  Upload,
  X,
} from "lucide-react";

import EditableText from "@/components/EditableText";
import GoogleAccountDialog from "@/components/GoogleAccountDialog";
import SheetImport from "@/components/SheetImport";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toCsv } from "@/lib/sheet";
import { cn } from "@/lib/utils";
import {
  applicationStatuses,
  createApplication,
  deleteApplication,
  googleStatus,
  pushApplicationToSheet,
  removeFromGoogleSheet,
  updateApplication,
  type Application,
  type Branch,
  type GoogleStatus,
  type NewApplication,
} from "@/rpc";

/// Statuses whose resume is finished with, and can be folded away.
const SETTLED = ["offer", "rejected"];

const SWATCH: Record<string, string> = {
  draft: "bg-status-draft/15 text-status-draft",
  applied: "bg-status-applied/20 text-status-applied",
  interview: "bg-status-interview/20 text-status-interview",
  offer: "bg-status-offer/20 text-status-offer",
  rejected: "bg-status-rejected/15 text-status-rejected",
};

export const BLANK_APPLICATION: NewApplication = {
  company: "",
  position: "",
  status: "draft",
  date_applied: null,
  listing_url: null,
  resume_branch: null,
  resume_drive_link: null,
  notes: null,
  jd_text: null,
};

function fields(application: Application): NewApplication {
  const { id: _id, created_at: _created, ...rest } = application;
  return rest;
}

function Cell({ children, className }: { children: React.ReactNode; className?: string }) {
  return <td className={cn("px-3 py-2.5 align-middle", className)}>{children}</td>;
}

/// `date_applied`'s value for sorting. A blank or unparseable date sorts as the earliest
/// possible date, the same way an empty string sorts first among the other columns.
function dateSortValue(value: string | null): number {
  const parsed = value ? Date.parse(value) : NaN;
  return Number.isNaN(parsed) ? -Infinity : parsed;
}

/// A key every column but the trailing delete button can sort by: alphabetically on its raw
/// string value, except `date_applied`, which sorts by the date it names.
type SortKey = keyof Pick<
  Application,
  | "company"
  | "position"
  | "date_applied"
  | "status"
  | "listing_url"
  | "resume_branch"
  | "resume_drive_link"
  | "notes"
  | "jd_text"
>;

function SortHeader({
  label,
  sort,
  column,
  onClick,
}: {
  label: string;
  sort: { key: SortKey; direction: "asc" | "desc" } | null;
  column: SortKey;
  onClick: () => void;
}) {
  const active = sort?.key === column;
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-0.5 hover:text-foreground"
    >
      {label}
      {active ? (
        sort.direction === "asc" ? (
          <ChevronUp className="size-3" />
        ) : (
          <ChevronDown className="size-3" />
        )
      ) : (
        <ChevronsUpDown className="size-3 opacity-30" />
      )}
    </button>
  );
}

export default function ApplicationTracker({
  applications,
  branches,
  onChanged,
}: {
  applications: Application[];
  branches: Branch[];
  onChanged: () => void;
}) {
  const [statuses, setStatuses] = useState<string[]>([]);
  const [hideSettled, setHideSettled] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sort, setSort] = useState<{ key: SortKey; direction: "asc" | "desc" } | null>(null);
  const [googleSync, setGoogleSync] = useState<GoogleStatus | null>(null);
  const [connectingGoogle, setConnectingGoogle] = useState(false);

  const refreshGoogleSync = () => googleStatus().then(setGoogleSync, () => setGoogleSync(null));

  useEffect(() => {
    applicationStatuses().then(setStatuses, () => setStatuses([]));
    refreshGoogleSync();
  }, []);

  // Every local edit below funnels through here: refreshes the sync status display, and, with
  // auto-sync on, pushes the edited application straight to its own sheet row. This pushes rather
  // than running the full bidirectional sync (google.sync_now) — that merge treats a non-blank
  // sheet cell as always winning, which is right for the background poll pulling in genuine
  // sheet-side edits, but would immediately read this application's still-stale sheet cell and
  // undo the edit just made here.
  const notifyChanged = (pushed?: Application | null) => {
    onChanged();
    if (googleSync?.sync_enabled && pushed) {
      pushApplicationToSheet(pushed).then(refreshGoogleSync, refreshGoogleSync);
    } else {
      refreshGoogleSync();
    }
  };

  const toggleSort = (key: SortKey) =>
    setSort((prev) =>
      prev?.key === key
        ? { key, direction: prev.direction === "asc" ? "desc" : "asc" }
        : { key, direction: "asc" },
    );

  const shown = useMemo(() => {
    const filtered = hideSettled
      ? applications.filter((one) => !SETTLED.includes(one.status))
      : applications;
    if (!sort) return filtered;
    const factor = sort.direction === "asc" ? 1 : -1;
    return [...filtered].sort((a, b) =>
      sort.key === "date_applied"
        ? factor * (dateSortValue(a.date_applied) - dateSortValue(b.date_applied))
        : factor * String(a[sort.key] ?? "").localeCompare(String(b[sort.key] ?? "")),
    );
  }, [applications, hideSettled, sort]);

  const counts = useMemo(() => {
    const tally = new Map<string, number>();
    for (const one of applications) {
      tally.set(one.status, (tally.get(one.status) ?? 0) + 1);
    }
    return tally;
  }, [applications]);

  const save = (application: Application, changes: Partial<NewApplication>) =>
    updateApplication(application.id, { ...fields(application), ...changes }).then(
      notifyChanged,
      (failure: Error) => setError(failure.message),
    );

  const add = () =>
    createApplication(BLANK_APPLICATION).then(notifyChanged, (failure: Error) =>
      setError(failure.message),
    );

  // Clears the sheet row before deleting locally — otherwise the next sync (which notifyChanged
  // may itself trigger) reads the row back with no local match and recreates the application.
  // Always called rather than gated on `googleSync` — that state can still be stale/null right
  // after mount, and the server-side handler already no-ops safely when nothing is connected.
  const remove = (application: Application) => {
    removeFromGoogleSheet(application.id, application.company, application.position)
      .catch(() => undefined)
      .then(() =>
        deleteApplication(application.id).then(notifyChanged, (failure: Error) =>
          setError(failure.message),
        ),
      );
  };

  return (
    <section className="rounded-lg border bg-card">
      <header className="flex flex-wrap items-center gap-2 border-b px-4 py-3">
        <h2 className="text-sm font-semibold">Applications</h2>

        <div className="flex flex-wrap items-center gap-1">
          {statuses.map((status) => (
            <span
              key={status}
              className={cn(
                "rounded-full px-1.5 py-0.5 text-[10px] font-medium tabular-nums",
                SWATCH[status],
              )}
            >
              {counts.get(status) ?? 0} {status}
            </span>
          ))}
        </div>

        <span className="flex-1" />

        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1.5 text-xs"
          onClick={() => setConnectingGoogle(true)}
          title="Google Sheets sync"
        >
          <span
            className={cn(
              "size-1.5 shrink-0 rounded-full",
              !googleSync?.connected
                ? "bg-muted-foreground"
                : googleSync.last_sync_error
                  ? "bg-status-rejected"
                  : googleSync.sync_enabled
                    ? "bg-status-offer"
                    : "bg-status-applied",
            )}
          />
          <SheetIcon className="size-3.5" />
          {!googleSync?.connected
            ? "Sheet"
            : googleSync.last_sync_error
              ? "Sync failed"
              : googleSync.sync_enabled
                ? "Synced"
                : "Connected"}
        </Button>
        <GoogleAccountDialog
          open={connectingGoogle}
          onOpenChange={(open) => {
            setConnectingGoogle(open);
            if (!open) refreshGoogleSync();
          }}
        />

        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs"
          onClick={() => setHideSettled(!hideSettled)}
        >
          {hideSettled ? "Show settled" : "Hide settled"}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1.5 text-xs"
          onClick={() => setImporting(true)}
        >
          <Upload className="size-3.5" />
          Import
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1.5 text-xs"
          disabled={applications.length === 0}
          onClick={() => {
            const csv = toCsv(applications.map(fields));
            const url = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
            const link = document.createElement("a");
            link.href = url;
            link.download = "applications.csv";
            link.click();
            URL.revokeObjectURL(url);
          }}
        >
          <Download className="size-3.5" />
          Export
        </Button>
        <Button variant="ghost" size="sm" className="h-7 gap-1.5 text-xs" onClick={add}>
          <Plus className="size-3.5" />
          Add
        </Button>
      </header>

      <div className="overflow-x-auto">
        <table className="w-full min-w-[64rem] text-sm">
          <thead>
            <tr className="border-b text-left text-xs tracking-wide text-muted-foreground uppercase">
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Company" sort={sort} column="company" onClick={() => toggleSort("company")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Position" sort={sort} column="position" onClick={() => toggleSort("position")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Applied" sort={sort} column="date_applied" onClick={() => toggleSort("date_applied")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Status" sort={sort} column="status" onClick={() => toggleSort("status")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Listing" sort={sort} column="listing_url" onClick={() => toggleSort("listing_url")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Branch" sort={sort} column="resume_branch" onClick={() => toggleSort("resume_branch")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Drive link" sort={sort} column="resume_drive_link" onClick={() => toggleSort("resume_drive_link")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Notes" sort={sort} column="notes" onClick={() => toggleSort("notes")} />
              </th>
              <th className="px-3 py-2 font-medium">
                <SortHeader label="Job description" sort={sort} column="jd_text" onClick={() => toggleSort("jd_text")} />
              </th>
              <th className="w-8" />
            </tr>
          </thead>
          <tbody>
            {shown.map((application) => (
              <tr key={application.id} className="group border-b last:border-b-0">
                <Cell className="min-w-40">
                  <EditableText
                    value={application.company}
                    onCommit={(company) => save(application, { company })}
                    placeholder="Company"
                  />
                </Cell>
                <Cell className="min-w-80">
                  <EditableText
                    value={application.position}
                    onCommit={(position) => save(application, { position })}
                    placeholder="Position"
                  />
                </Cell>
                <Cell className="w-32">
                  <EditableText
                    value={application.date_applied ?? ""}
                    onCommit={(date) => save(application, { date_applied: date || null })}
                    placeholder="YYYY-MM-DD"
                    className="tabular-nums"
                  />
                </Cell>
                <Cell className="w-36">
                  <Select
                    value={application.status}
                    onValueChange={(status) => status && save(application, { status })}
                  >
                    <SelectTrigger
                      size="sm"
                      className={cn(
                        "h-7 w-full border-transparent text-sm shadow-none",
                        SWATCH[application.status],
                      )}
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {statuses.map((status) => (
                        <SelectItem key={status} value={status} className="text-sm">
                          {status}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Cell>
                <Cell className="max-w-48">
                  <div className="flex items-center gap-1">
                    <EditableText
                      value={application.listing_url ?? ""}
                      onCommit={(url) => save(application, { listing_url: url || null })}
                      placeholder="URL"
                      className="truncate"
                      title={application.listing_url ?? undefined}
                    />
                    {application.listing_url && (
                      <a
                        href={application.listing_url}
                        target="_blank"
                        rel="noreferrer"
                        className="shrink-0 text-muted-foreground hover:text-foreground"
                        title="Open listing"
                      >
                        <ExternalLink className="size-3.5" />
                      </a>
                    )}
                  </div>
                </Cell>
                <Cell className="max-w-40">
                  <Select
                    value={application.resume_branch ?? ""}
                    onValueChange={(branch) =>
                      save(application, { resume_branch: branch || null })
                    }
                  >
                    <SelectTrigger
                      size="sm"
                      className="h-7 w-full border-transparent text-sm shadow-none hover:border-border"
                      title={application.resume_branch ?? undefined}
                    >
                      <SelectValue placeholder="—" className="truncate" />
                    </SelectTrigger>
                    <SelectContent>
                      {branches.map((branch) => (
                        <SelectItem
                          key={branch.name}
                          value={branch.name}
                          className="font-mono text-sm"
                        >
                          {branch.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Cell>
                <Cell className="max-w-48">
                  <div className="flex items-center gap-1">
                    <EditableText
                      value={application.resume_drive_link ?? ""}
                      onCommit={(link) =>
                        save(application, { resume_drive_link: link || null })
                      }
                      placeholder="Drive link"
                      className="truncate"
                      title={application.resume_drive_link ?? undefined}
                    />
                    {application.resume_drive_link && (
                      <a
                        href={application.resume_drive_link}
                        target="_blank"
                        rel="noreferrer"
                        className="shrink-0 text-muted-foreground hover:text-foreground"
                        title="Open in Drive"
                      >
                        <ExternalLink className="size-3.5" />
                      </a>
                    )}
                  </div>
                </Cell>
                <Cell className="min-w-56">
                  <EditableText
                    value={application.notes ?? ""}
                    onCommit={(notes) => save(application, { notes: notes || null })}
                    placeholder="—"
                  />
                </Cell>
                <Cell className="min-w-48">
                  <EditableText
                    value={application.jd_text ?? ""}
                    onCommit={(jd_text) => save(application, { jd_text: jd_text || null })}
                    placeholder="Paste the posting"
                    multiline
                  />
                </Cell>
                <Cell>
                  <button
                    type="button"
                    aria-label={`Remove ${application.company || "application"}`}
                    title="Remove"
                    className="grid size-6 place-items-center rounded-sm text-muted-foreground opacity-0 hover:bg-destructive/15 hover:text-destructive group-hover:opacity-100"
                    onClick={() => remove(application)}
                  >
                    <X className="size-3" />
                  </button>
                </Cell>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {shown.length === 0 && (
        <p className="px-4 py-6 text-center text-sm text-muted-foreground">
          {applications.length === 0
            ? "No applications yet."
            : "Every application is settled."}
        </p>
      )}

      {error && (
        <p className="border-t px-4 py-2 font-mono text-xs text-destructive">{error}</p>
      )}

      <SheetImport
        open={importing}
        onOpenChange={setImporting}
        onImported={notifyChanged}
      />
    </section>
  );
}
