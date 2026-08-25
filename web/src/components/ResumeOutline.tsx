import { useCallback, useEffect, useState } from "react";
import { ChevronRight, Minus } from "lucide-react";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { entryLabel } from "@/entryFields";
import {
  carriesEntry,
  carriesMove,
  carriesSection,
  carriesWording,
  draggedEntry,
  draggedMove,
  draggedSection,
  draggedWording,
  joinElements,
  splitElements,
  startMoveDrag,
  startWordingDrag,
} from "@/lib/placement";
import { cn } from "@/lib/utils";
import {
  getEntry,
  placeBullet,
  placeEntry,
  removeResumeBullet,
  moveResumeBullet,
  moveResumeEntry,
  placeSection,
  removeResumeEntry,
  resumeOutline,
  setResumeEntryField,
  type ResumeEntry,
  type ResumeSection,
} from "@/rpc";

// A resume entry carries no type, so its shape names it.
const TYPE_BY_TITLE_KEY: Record<string, string> = {
  company: "experience",
  institution: "education",
  title: "publication",
  label: "one-line",
  name: "normal",
};

function shapeOf(fields: unknown): string {
  const held = (fields ?? {}) as Record<string, unknown>;
  const key = Object.keys(TYPE_BY_TITLE_KEY).find(
    (candidate) => held[candidate] !== undefined,
  );
  return key ? TYPE_BY_TITLE_KEY[key] : "normal";
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="px-3 py-1.5 text-xs text-muted-foreground">{children}</p>;
}

function RemoveButton({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="grid size-4 shrink-0 place-items-center rounded-sm text-muted-foreground opacity-0 hover:bg-destructive/15 hover:text-destructive group-hover:opacity-100 focus-visible:opacity-100"
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
    >
      <Minus className="size-3" />
    </button>
  );
}

// A one-line entry keeps its elements in one comma-separated string; each is shown and
// dragged on its own, and rewriting the string is what adds, removes, or reorders one.
function ElementRow({
  details,
  onChange,
}: {
  details: string;
  onChange: (details: string) => void;
}) {
  const elements = splitElements(details);
  const [target, setTarget] = useState<number | null>(null);

  const move = (from: number, to: number) => {
    const next = [...elements];
    const [moved] = next.splice(from, 1);
    next.splice(to > from ? to - 1 : to, 0, moved);
    onChange(joinElements(next));
  };

  return (
    <div className="flex flex-wrap items-center gap-1 py-1 pr-2 pl-9">
      {elements.map((element, index) => (
        <span
          key={`${element}-${index}`}
          draggable
          onDragStart={(event) => {
            event.dataTransfer.setData("text/x-element-index", String(index));
            startWordingDrag(event, element);
          }}
          onDragOver={(event) => {
            event.preventDefault();
            setTarget(index);
          }}
          onDragLeave={() => setTarget(null)}
          onDrop={(event) => {
            event.preventDefault();
            event.stopPropagation();
            setTarget(null);
            const from = Number(event.dataTransfer.getData("text/x-element-index"));
            if (Number.isInteger(from)) return move(from, index);
            const wording = draggedWording(event);
            if (wording) onChange(joinElements([...elements, wording]));
          }}
          className={cn(
            "group flex cursor-grab items-center gap-1 rounded-sm border px-1.5 py-0.5 text-xs active:cursor-grabbing",
            target === index && "border-ring bg-muted",
          )}
        >
          {element}
          <RemoveButton
            label={`Remove ${element}`}
            onClick={() =>
              onChange(joinElements(elements.filter((_, at) => at !== index)))
            }
          />
        </span>
      ))}
    </div>
  );
}

function EntryNode({
  section,
  entry,
  onPlaceWording,
  onRemoveEntry,
  onRemoveWording,
  onSetField,
  onMoveEntry,
  onMoveWording,
}: {
  section: string;
  entry: ResumeEntry;
  onPlaceWording: (section: string, index: number, text: string) => void;
  onRemoveEntry: (section: string, index: number) => void;
  onRemoveWording: (section: string, index: number, at: number) => void;
  onSetField: (section: string, index: number, key: string, value: unknown) => void;
  onMoveEntry: (section: string, from: number, to: number) => void;
  onMoveWording: (section: string, index: number, from: number, to: number) => void;
}) {
  const [open, setOpen] = useState(true);
  const [over, setOver] = useState(false);
  const [wordingOver, setWordingOver] = useState<number | null>(null);
  const shape = shapeOf(entry.fields);
  const { title, subtitle, dates } = entryLabel(shape, entry.fields);
  const details = ((entry.fields ?? {}) as Record<string, unknown>).details;
  const isOneLine = shape === "one-line";

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className={cn("border-b", over && "bg-muted/60 ring-1 ring-ring ring-inset")}
      onDragOver={(event: React.DragEvent) => {
        if (!carriesWording(event) && !carriesMove(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = carriesMove(event) ? "move" : "copy";
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event: React.DragEvent) => {
        setOver(false);
        const from = draggedMove(event);
        if (from !== null) {
          event.preventDefault();
          event.stopPropagation();
          return onMoveEntry(section, from, entry.index);
        }
        const text = draggedWording(event);
        if (!text) return;
        event.preventDefault();
        setOpen(true);
        if (isOneLine) {
          const current = typeof details === "string" ? details : "";
          onSetField(
            section,
            entry.index,
            "details",
            joinElements([...splitElements(current), text]),
          );
        } else {
          onPlaceWording(section, entry.index, text);
        }
      }}
    >
      <div
        className="group flex items-baseline gap-1.5 pr-2"
        draggable
        onDragStart={(event: React.DragEvent) => {
          event.stopPropagation();
          startMoveDrag(event, entry.index);
        }}
      >
        <CollapsibleTrigger className="flex flex-1 cursor-grab items-baseline gap-1.5 py-1 pl-6 text-left hover:bg-muted/50 active:cursor-grabbing">
          <ChevronRight className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[panel-open]:rotate-90" />
          <span className="truncate text-xs font-medium">{title}</span>
          {subtitle && !isOneLine && (
            <span className="truncate text-xs text-muted-foreground">{subtitle}</span>
          )}
          <span className="flex-1" />
          {dates && (
            <span className="shrink-0 text-[10px] text-muted-foreground tabular-nums">
              {dates}
            </span>
          )}
        </CollapsibleTrigger>
        <RemoveButton
          label={`Remove ${title}`}
          onClick={() => onRemoveEntry(section, entry.index)}
        />
      </div>

      <CollapsibleContent>
        {isOneLine ? (
          <ElementRow
            details={typeof details === "string" ? details : ""}
            onChange={(next) => onSetField(section, entry.index, "details", next)}
          />
        ) : entry.highlights.length === 0 ? (
          <p className="py-1 pr-2 pl-9 text-xs text-muted-foreground italic">
            Drop a wording here.
          </p>
        ) : (
          entry.highlights.map((highlight, at) => (
            <div
              key={at}
              draggable
              onDragStart={(event) => {
                event.stopPropagation();
                startMoveDrag(event, at);
              }}
              onDragOver={(event) => {
                if (!carriesMove(event)) return;
                event.preventDefault();
                event.stopPropagation();
                setWordingOver(at);
              }}
              onDragLeave={() => setWordingOver(null)}
              onDrop={(event) => {
                const from = draggedMove(event);
                setWordingOver(null);
                if (from === null) return;
                event.preventDefault();
                event.stopPropagation();
                onMoveWording(section, entry.index, from, at);
              }}
              className={cn(
                "group flex cursor-grab gap-1.5 py-1 pr-2 pl-9 active:cursor-grabbing",
                wordingOver === at && "bg-muted",
              )}
            >
              <span className="mt-1.5 ml-1 size-1 shrink-0 rounded-full bg-muted-foreground" />
              <span className="flex-1 text-xs leading-snug">{highlight}</span>
              <RemoveButton
                label="Remove this wording"
                onClick={() => onRemoveWording(section, entry.index, at)}
              />
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
  const [over, setOver] = useState<string | null>(null);

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

  const run = useCallback(
    (work: Promise<unknown>) =>
      work.then(load, (failure: Error) => setError(failure.message)),
    [load],
  );

  const dropEntry = useCallback(
    (section: string, entryId: number) => {
      run(
        getEntry(entryId).then((stored) => {
          if (!stored) throw new Error(`entry ${entryId} is no longer in the store`);
          return placeEntry(section, stored.fields);
        }),
      );
    },
    [run],
  );

  if (error) return <Note>{error}</Note>;
  if (!sections) return <Note>Loading…</Note>;

  return (
    <div
      className="min-h-full"
      onDragOver={(event) => {
        if (!carriesSection(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
      }}
      onDrop={(event) => {
        const name = draggedSection(event);
        if (!name) return;
        event.preventDefault();
        run(placeSection(name));
      }}
    >
      {sections.length === 0 && (
        <Note>Drag a Section across to start this resume.</Note>
      )}
      {sections.map((section) => (
        <div
          key={section.name}
          onDragOver={(event) => {
            if (!carriesEntry(event)) return;
            event.preventDefault();
            event.dataTransfer.dropEffect = "copy";
            setOver(section.name);
          }}
          onDragLeave={() => setOver(null)}
          onDrop={(event) => {
            const id = draggedEntry(event);
            setOver(null);
            if (id === null) return;
            event.preventDefault();
            dropEntry(section.name, id);
          }}
          className={cn(over === section.name && "bg-muted/40")}
        >
          <div className="flex items-center gap-1.5 border-b px-2 py-1.5">
            <span className="text-xs font-semibold">{section.name}</span>
          </div>
          {section.entries.map((entry) => (
            <EntryNode
              key={`${entry.index}-${JSON.stringify(entry.fields)}`}
              section={section.name}
              entry={entry}
              onPlaceWording={(s, i, text) => run(placeBullet(s, i, text))}
              onRemoveEntry={(s, i) => run(removeResumeEntry(s, i))}
              onRemoveWording={(s, i, at) => run(removeResumeBullet(s, i, at))}
              onSetField={(s, i, key, value) =>
                run(setResumeEntryField(s, i, key, value))
              }
              onMoveEntry={(s, from, to) => run(moveResumeEntry(s, from, to))}
              onMoveWording={(s, i, from, to) =>
                run(moveResumeBullet(s, i, from, to))
              }
            />
          ))}
        </div>
      ))}
    </div>
  );
}
