import { useCallback, useEffect, useState } from "react";
import { ChevronDown, ChevronUp, RefreshCw } from "lucide-react";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { type Async } from "@/lib/async";
import {
  AI_UNCONFIGURED_CODE,
  RpcFailure,
  assessResume,
  listJobDescriptions,
  matchJobDescription,
  type Assessment,
  type JobDescription,
  type Keyword,
  type MatchReport,
} from "@/rpc";

function Chip({ children, muted = false }: { children: React.ReactNode; muted?: boolean }) {
  return (
    <span
      className={
        "inline-block rounded-full border px-2 py-0.5 text-xs " +
        (muted ? "text-muted-foreground" : "border-foreground/30")
      }
    >
      {children}
    </span>
  );
}

function KeywordList({ keywords, muted }: { keywords: Keyword[]; muted?: boolean }) {
  if (keywords.length === 0) return <p className="text-xs text-muted-foreground">None.</p>;
  return (
    <div className="flex flex-wrap gap-1">
      {keywords.map((k) => (
        <Chip key={k.term} muted={muted}>
          {k.term}
        </Chip>
      ))}
    </div>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="flex min-w-0 flex-1 flex-col border-r last:border-r-0"
    >
      <CollapsibleTrigger className="flex shrink-0 items-center justify-between px-3 py-1.5 text-xs font-medium hover:bg-muted/50">
        <span>{title}</span>
        {open ? <ChevronDown className="size-3.5" /> : <ChevronUp className="size-3.5" />}
      </CollapsibleTrigger>
      <CollapsibleContent className="min-h-0 flex-1">
        <ScrollArea className="h-full px-3 pb-2">{children}</ScrollArea>
      </CollapsibleContent>
    </Collapsible>
  );
}

function MatchSection({ jdId, state }: { jdId: number | null; state: Async<MatchReport> | null }) {
  if (jdId === null || !state) {
    return <p className="text-xs text-muted-foreground">Pick a job description above.</p>;
  }
  if (state.status === "loading") return <p className="text-xs text-muted-foreground">Scoring…</p>;
  if (state.status === "error") return <p className="text-xs text-muted-foreground">{state.message}</p>;
  const report = state.data;
  return (
    <div className="flex flex-col gap-3 text-sm">
      <p>
        <span className="text-2xl font-semibold">{report.score}</span>
        <span className="text-muted-foreground"> / 100 keyword match</span>
      </p>
      <div>
        <p className="mb-1 text-xs font-medium text-muted-foreground">Matched</p>
        <KeywordList keywords={report.matched} />
      </div>
      <div>
        <p className="mb-1 text-xs font-medium text-muted-foreground">Missing</p>
        <KeywordList keywords={report.missing} muted />
      </div>
      {report.writing_notes.length > 0 && (
        <div>
          <p className="mb-1 text-xs font-medium text-muted-foreground">Writing notes</p>
          <ul className="flex flex-col gap-1">
            {report.writing_notes.map((note, i) => (
              <li key={i} className="text-xs">
                <span className="text-muted-foreground">
                  {note.kind === "unquantified" ? "No number: " : "Weak opener: "}
                </span>
                {note.line}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function AssessmentSection({
  jdId,
  state,
}: {
  jdId: number | null;
  state: Async<Assessment> | null;
}) {
  if (jdId === null || !state) {
    return <p className="text-xs text-muted-foreground">Pick a job description above.</p>;
  }
  if (state.status === "loading") return <p className="text-xs text-muted-foreground">Asking Claude…</p>;
  if (state.status === "error") return <p className="text-xs text-muted-foreground">{state.message}</p>;
  const assessment = state.data;

  const scores: [string, Assessment["relevance"]][] = [
    ["Relevance", assessment.relevance],
    ["Tone", assessment.tone],
    ["Flow", assessment.flow],
  ];

  return (
    <div className="flex flex-col gap-3 text-sm">
      <div className="flex flex-wrap gap-4">
        {scores.map(([label, score]) => (
          <div key={label}>
            <p className="text-xs font-medium text-muted-foreground">{label}</p>
            <p>
              <span className="text-lg font-semibold">{score.score}</span> / 100
            </p>
            <p className="text-xs text-muted-foreground">{score.reason}</p>
          </div>
        ))}
      </div>
      {assessment.semantic_coverage.some((m) => m.covered) && (
        <div>
          <p className="mb-1 text-xs font-medium text-muted-foreground">
            Missing keywords actually covered
          </p>
          <ul className="flex flex-col gap-2">
            {assessment.semantic_coverage
              .filter((m) => m.covered)
              .map((m) => (
                <li key={m.keyword} className="text-xs">
                  <span className="font-medium">{m.keyword}</span>
                  {m.evidence && <span className="text-muted-foreground"> — {m.evidence}</span>}
                </li>
              ))}
          </ul>
        </div>
      )}
      {assessment.suggestions.length > 0 && (
        <div>
          <p className="mb-1 text-xs font-medium text-muted-foreground">Suggestions</p>
          <ul className="flex flex-col gap-2">
            {assessment.suggestions.map((s, i) => (
              <li key={i} className="text-xs">
                <p className="text-muted-foreground line-through">{s.on}</p>
                <p>{s.suggestion}</p>
                <p className="text-muted-foreground">{s.why}</p>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function assessErrorMessage(error: unknown): string {
  if (error instanceof RpcFailure && error.code === AI_UNCONFIGURED_CODE) {
    return "AI isn't configured — set ANTHROPIC_API_KEY to enable this.";
  }
  return error instanceof Error ? error.message : "the assessment failed";
}

export default function TailoringPanel() {
  const [jobDescriptions, setJobDescriptions] = useState<JobDescription[]>([]);
  const [jdId, setJdId] = useState<number | null>(null);
  const [match, setMatch] = useState<Async<MatchReport> | null>(null);
  const [assessment, setAssessment] = useState<Async<Assessment> | null>(null);

  useEffect(() => {
    listJobDescriptions().then(setJobDescriptions, () => setJobDescriptions([]));
  }, []);

  const refresh = useCallback((id: number) => {
    setMatch({ status: "loading" });
    matchJobDescription(id).then(
      (report) => setMatch({ status: "ok", data: report }),
      (error: unknown) =>
        setMatch({
          status: "error",
          message: error instanceof Error ? error.message : "could not score the match",
        }),
    );

    setAssessment({ status: "loading" });
    assessResume(id).then(
      (result) => setAssessment({ status: "ok", data: result }),
      (error: unknown) => setAssessment({ status: "error", message: assessErrorMessage(error) }),
    );
  }, []);

  useEffect(() => {
    if (jdId !== null) refresh(jdId);
  }, [jdId, refresh]);

  return (
    <div className="flex h-full min-h-0 flex-col bg-background">
      <div className="flex shrink-0 items-center gap-2 border-b px-3 py-1.5">
        <span className="text-xs font-medium text-muted-foreground">Tailor against</span>
        <Select
          value={jdId?.toString() ?? undefined}
          onValueChange={(value) => setJdId(Number(value))}
        >
          <SelectTrigger size="sm" className="h-7 w-64">
            <SelectValue>
              {(value: string | null) => {
                const jd = jobDescriptions.find((j) => j.id.toString() === value);
                return jd ? (jd.title ?? jd.text.slice(0, 60)) : "Pick a job description…";
              }}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            {jobDescriptions.map((jd) => (
              <SelectItem key={jd.id} value={jd.id.toString()}>
                {jd.title ?? jd.text.slice(0, 60)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {jdId !== null && (
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            onClick={() => refresh(jdId)}
            disabled={assessment?.status === "loading"}
            aria-label="Refresh"
            title="Refresh"
          >
            <RefreshCw className="size-3.5" />
          </Button>
        )}
      </div>
      <div className="flex min-h-0 flex-1">
        <Section title="Match">
          <MatchSection jdId={jdId} state={match} />
        </Section>
        <Section title="Assessment">
          <AssessmentSection jdId={jdId} state={assessment} />
        </Section>
      </div>
    </div>
  );
}
