import { useEffect, useMemo, useState } from "react";
import { Download, ExternalLink, Plus, Upload, X } from "lucide-react";

import EditableText from "@/components/EditableText";
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
  updateApplication,
  type Application,
  type Branch,
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

function fields(application: Application): NewApplication {
  const { id: _id, created_at: _created, ...rest } = application;
  return rest;
}

function Cell({ children, className }: { children: React.ReactNode; className?: string }) {
  return <td className={cn("px-2 py-1.5 align-middle", className)}>{children}</td>;
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

  useEffect(() => {
    applicationStatuses().then(setStatuses, () => setStatuses([]));
  }, []);

  const shown = useMemo(
    () =>
      hideSettled
        ? applications.filter((one) => !SETTLED.includes(one.status))
        : applications,
    [applications, hideSettled],
  );

  const counts = useMemo(() => {
    const tally = new Map<string, number>();
    for (const one of applications) {
      tally.set(one.status, (tally.get(one.status) ?? 0) + 1);
    }
    return tally;
  }, [applications]);

  const save = (application: Application, changes: Partial<NewApplication>) =>
    updateApplication(application.id, { ...fields(application), ...changes }).then(
      onChanged,
      (failure: Error) => setError(failure.message),
    );

  const add = () =>
    createApplication({
      company: "",
      position: "",
      status: "draft",
      date_applied: null,
      listing_url: null,
      resume_branch: null,
      notes: null,
      jd_text: null,
    }).then(onChanged, (failure: Error) => setError(failure.message));

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
        <table className="w-full min-w-[52rem] text-xs">
          <thead>
            <tr className="border-b text-left text-[10px] tracking-wide text-muted-foreground uppercase">
              <th className="px-2 py-1.5 font-medium">Company</th>
              <th className="px-2 py-1.5 font-medium">Position</th>
              <th className="px-2 py-1.5 font-medium">Applied</th>
              <th className="px-2 py-1.5 font-medium">Status</th>
              <th className="px-2 py-1.5 font-medium">Listing</th>
              <th className="px-2 py-1.5 font-medium">Resume</th>
              <th className="px-2 py-1.5 font-medium">Notes</th>
              <th className="px-2 py-1.5 font-medium">Job description</th>
              <th className="w-8" />
            </tr>
          </thead>
          <tbody>
            {shown.map((application) => (
              <tr key={application.id} className="group border-b last:border-b-0">
                <Cell className="min-w-32">
                  <EditableText
                    value={application.company}
                    onCommit={(company) => save(application, { company })}
                    placeholder="Company"
                  />
                </Cell>
                <Cell className="min-w-48">
                  <EditableText
                    value={application.position}
                    onCommit={(position) => save(application, { position })}
                    placeholder="Position"
                  />
                </Cell>
                <Cell className="w-28">
                  <EditableText
                    value={application.date_applied ?? ""}
                    onCommit={(date) => save(application, { date_applied: date || null })}
                    placeholder="YYYY-MM-DD"
                    className="tabular-nums"
                  />
                </Cell>
                <Cell className="w-32">
                  <Select
                    value={application.status}
                    onValueChange={(status) => status && save(application, { status })}
                  >
                    <SelectTrigger
                      size="sm"
                      className={cn(
                        "h-6 w-full border-transparent text-xs shadow-none",
                        SWATCH[application.status],
                      )}
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {statuses.map((status) => (
                        <SelectItem key={status} value={status} className="text-xs">
                          {status}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Cell>
                <Cell className="max-w-40">
                  <div className="flex items-center gap-1">
                    <EditableText
                      value={application.listing_url ?? ""}
                      onCommit={(url) => save(application, { listing_url: url || null })}
                      placeholder="URL"
                      className="truncate"
                    />
                    {application.listing_url && (
                      <a
                        href={application.listing_url}
                        target="_blank"
                        rel="noreferrer"
                        className="shrink-0 text-muted-foreground hover:text-foreground"
                        title="Open listing"
                      >
                        <ExternalLink className="size-3" />
                      </a>
                    )}
                  </div>
                </Cell>
                <Cell className="w-48">
                  <Select
                    value={application.resume_branch ?? ""}
                    onValueChange={(branch) =>
                      save(application, { resume_branch: branch || null })
                    }
                  >
                    <SelectTrigger
                      size="sm"
                      className="h-6 w-full border-transparent text-xs shadow-none hover:border-border"
                    >
                      <SelectValue placeholder="—" />
                    </SelectTrigger>
                    <SelectContent>
                      {branches.map((branch) => (
                        <SelectItem
                          key={branch.name}
                          value={branch.name}
                          className="font-mono text-xs"
                        >
                          {branch.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Cell>
                <Cell className="min-w-40">
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
                    className="grid size-5 place-items-center rounded-sm text-muted-foreground opacity-0 hover:bg-destructive/15 hover:text-destructive group-hover:opacity-100"
                    onClick={() =>
                      deleteApplication(application.id).then(onChanged, (failure: Error) =>
                        setError(failure.message),
                      )
                    }
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
        onImported={onChanged}
      />
    </section>
  );
}
