import { useCallback, useEffect, useState } from "react";
import { ChevronRight } from "lucide-react";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { entryLabel } from "@/entryFields";
import { carriesWording, draggedWording } from "@/lib/placement";
import { cn } from "@/lib/utils";
import {
  placeBullet,
  resumeOutline,
  type ResumeEntry,
  type ResumeSection,
} from "@/rpc";

// A resume entry carries no type, so its title is whichever of these keys it has.
const TITLE_KEYS = ["company", "name", "institution", "title", "label"];

function titleOf(fields: unknown): { title: string; subtitle: string; dates: string } {
  const held = (fields ?? {}) as Record<string, unknown>;
  const key = TITLE_KEYS.find((candidate) => held[candidate] !== undefined);
  return entryLabel(
    key === "company"
      ? "experience"
      : key === "institution"
        ? "education"
        : key === "title"
          ? "publication"
          : key === "label"
            ? "one-line"
            : "normal",
    fields,
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="px-3 py-1.5 text-xs text-muted-foreground">{children}</p>;
}

function EntryNode({
  section,
  entry,
  onPlace,
}: {
  section: string;
  entry: ResumeEntry;
  onPlace: (section: string, index: number, text: string) => void;
}) {
  const [open, setOpen] = useState(true);
  const [over, setOver] = useState(false);
  const { title, subtitle, dates } = titleOf(entry.fields);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className={cn(
        "border-b border-dashed border-transparent",
        over && "border-solid border-ring bg-muted/60",
      )}
      onDragOver={(event: React.DragEvent) => {
        if (!carriesWording(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event: React.DragEvent) => {
        const text = draggedWording(event);
        setOver(false);
        if (!text) return;
        event.preventDefault();
        setOpen(true);
        onPlace(section, entry.index, text);
      }}
    >
      <CollapsibleTrigger className="group flex w-full items-baseline gap-1.5 py-1 pr-2 pl-6 text-left hover:bg-muted/50">
        <ChevronRight className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-panel-open:rotate-90" />
        <span className="truncate text-xs font-medium">{title}</span>
        {subtitle && (
          <span className="truncate text-xs text-muted-foreground">{subtitle}</span>
        )}
        <span className="flex-1" />
        {dates && (
          <span className="shrink-0 text-[10px] text-muted-foreground tabular-nums">
            {dates}
          </span>
        )}
      </CollapsibleTrigger>

      <CollapsibleContent>
        {entry.highlights.length === 0 ? (
          <p className="py-1 pr-2 pl-9 text-xs text-muted-foreground italic">
            Drop a wording here.
          </p>
        ) : (
          entry.highlights.map((highlight, position) => (
            <div key={position} className="flex gap-1.5 py-1 pr-2 pl-9">
              <span className="mt-1.5 ml-1 size-1 shrink-0 rounded-full bg-muted-foreground" />
              <span className="flex-1 text-xs leading-snug">{highlight}</span>
            </div>
          ))
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}

export default function ResumeOutline() {
  const [sections, setSections] = useState<ResumeSection[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    resumeOutline().then(
      (loaded) => {
        setSections(loaded);
        setError(null);
      },
      (failure: Error) => setError(failure.message),
    );
  }, []);

  useEffect(load, [load]);

  const place = useCallback(
    (section: string, index: number, text: string) => {
      placeBullet(section, index, text).then(load, (failure: Error) =>
        setError(failure.message),
      );
    },
    [load],
  );

  if (error) return <Note>{error}</Note>;
  if (!sections) return <Note>Loading…</Note>;
  if (sections.length === 0) return <Note>This resume has no sections.</Note>;

  return (
    <div>
      {sections.map((section) => (
        <div key={section.name}>
          <div className="flex items-center gap-1.5 border-b px-2 py-1.5">
            <span className="text-xs font-semibold">{section.name}</span>
          </div>
          {section.entries.map((entry) => (
            <EntryNode
              key={entry.index}
              section={section.name}
              entry={entry}
              onPlace={place}
            />
          ))}
        </div>
      ))}
    </div>
  );
}
