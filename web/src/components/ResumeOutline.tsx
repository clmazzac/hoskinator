import { useCallback, useEffect, useState } from "react";
import { BookmarkPlus, ChevronRight, Minus } from "lucide-react";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import SaveToBank from "@/components/SaveToBank";
import { entryLabel } from "@/entryFields";
import {
  carriesElement,
  carriesEntryMove,
  carriesEntryOfType,
  carriesSection,
  carriesSectionMove,
  carriesWording,
  carriesWordingMove,
  draggedElement,
  draggedEntry,
  draggedEntryMove,
  draggedSection,
  draggedSectionMove,
  draggedWording,
  draggedWordingMove,
  joinElements,
  splitElements,
  startElementDrag,
  startEntryMoveDrag,
  startSectionMoveDrag,
  startWordingMoveDrag,
} from "@/lib/placement";
import { resumeStep, useReloadOnHistory } from "@/lib/history";
import { cn } from "@/lib/utils";
import {
  getEntry,
  listSections,
  placeBullet,
  placeEntry,
  removeResumeBullet,
  moveResumeBullet,
  moveResumeEntry,
  moveResumeSection,
  placeSection,
  removeResumeEntry,
  resumeOutline,
  setResumeEntryField,
  type ResumeEntry,
  type ResumeSection,
} from "@/rpc";

/// Where an insertion will land, drawn between two rows rather than on top of either — a
/// misdrop onto the wrong row is exactly how a mismatched entry used to end up in a section.
function DropLine() {
  return (
    <div className="pointer-events-none relative z-10 h-0" aria-hidden>
      <div className="absolute inset-x-1 top-0 flex -translate-y-1/2 items-center">
        <svg width="7" height="7" viewBox="0 0 7 7" className="shrink-0 fill-foreground">
          <path d="M0 0 L7 3.5 L0 7 Z" />
        </svg>
        <div className="h-[2px] flex-1 rounded-full bg-foreground" />
      </div>
    </div>
  );
}

/// Whether the pointer sits in the top or bottom half of a row being dragged over.
function edgeOf(event: React.DragEvent): "before" | "after" {
  const box = event.currentTarget.getBoundingClientRect();
  return event.clientY < box.top + box.height / 2 ? "before" : "after";
}

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
  entryIndex,
  details,
  onChange,
}: {
  entryIndex: number;
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
            event.stopPropagation();
            startElementDrag(event, { entry: entryIndex, at: index }, element);
          }}
          onDragOver={(event) => {
            if (!carriesElement(event) && !carriesWording(event)) return;
            event.preventDefault();
            event.stopPropagation();
            setTarget(index);
          }}
          onDragLeave={(event) => {
            if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
            setTarget(null);
          }}
          onDrop={(event) => {
            setTarget(null);
            const from = draggedElement(event);
            if (from?.entry === entryIndex) {
              event.preventDefault();
              event.stopPropagation();
              return move(from.at, index);
            }
            const wording = draggedWording(event);
            if (!wording) return;
            event.preventDefault();
            event.stopPropagation();
            onChange(joinElements([...elements, wording]));
          }}
          className={cn(
            "relative group flex cursor-grab items-center gap-1 rounded-sm border px-1.5 py-0.5 text-xs active:cursor-grabbing",
          )}
        >
          {target === index && (
            <span
              aria-hidden
              className="absolute top-0 -left-[3px] h-full w-[2px] rounded-full bg-foreground"
            />
          )}
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
  onKeep,
}: {
  section: string;
  entry: ResumeEntry;
  onPlaceWording: (section: string, index: number, text: string) => void;
  onRemoveEntry: (section: string, index: number) => void;
  onRemoveWording: (section: string, index: number, at: number) => void;
  onSetField: (section: string, index: number, key: string, value: unknown) => void;
  onMoveEntry: (section: string, from: number, to: number) => void;
  onMoveWording: (section: string, index: number, from: number, to: number) => void;
  onKeep: (text: string) => void;
}) {
  const [open, setOpen] = useState(true);
  const [wordingGlow, setWordingGlow] = useState(false);
  const [edge, setEdge] = useState<"before" | "after" | null>(null);
  const [wordingAt, setWordingAt] = useState<number | null>(null);
  const shape = shapeOf(entry.fields);
  const { title, subtitle, dates } = entryLabel(shape, entry.fields);
  const details = ((entry.fields ?? {}) as Record<string, unknown>).details;
  const isOneLine = shape === "one-line";

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className={cn(
        "border-b",
        wordingGlow && "bg-muted/60 ring-1 ring-ring ring-inset",
      )}
      onDragOver={(event: React.DragEvent) => {
        // A sibling entry moves to the edge the pointer rests on; a bank wording always
        // appends, so it lights the whole entry rather than promising a position.
        if (carriesEntryMove(event)) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setEdge(edgeOf(event));
          return;
        }
        if (!carriesWording(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        setWordingGlow(true);
      }}
      onDragLeave={(event: React.DragEvent) => {
        // dragleave bubbles from children the pointer merely moved between.
        if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
        setWordingGlow(false);
        setEdge(null);
      }}
      onDrop={(event: React.DragEvent) => {
        const heldEdge = edge;
        setWordingGlow(false);
        setEdge(null);
        const from = draggedEntryMove(event);
        if (from !== null) {
          event.preventDefault();
          event.stopPropagation();
          const to = heldEdge === "after" ? entry.index + 1 : entry.index;
          if (to !== from) return onMoveEntry(section, from, to);
          return;
        }
        // An element of this entry that missed every chip is a reorder that landed nowhere.
        if (draggedElement(event)?.entry === entry.index) return;
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
      {edge === "before" && <DropLine />}
      <div
        className="group flex items-baseline gap-1.5 pr-2"
        draggable
        onDragStart={(event: React.DragEvent) => {
          event.stopPropagation();
          startEntryMoveDrag(event, entry.index);
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
            entryIndex={entry.index}
            details={typeof details === "string" ? details : ""}
            onChange={(next) => onSetField(section, entry.index, "details", next)}
          />
        ) : entry.highlights.length === 0 ? (
          <p className="py-1 pr-2 pl-9 text-xs text-muted-foreground italic">
            Drop a wording here.
          </p>
        ) : (
          <>
            {entry.highlights.map((highlight, at) => (
              <div key={at}>
                {wordingAt === at && <DropLine />}
                <div
                  draggable
                  onDragStart={(event) => {
                    event.stopPropagation();
                    startWordingMoveDrag(event, { entry: entry.index, at });
                  }}
                  onDragOver={(event) => {
                    if (!carriesWordingMove(event)) return;
                    event.preventDefault();
                    event.stopPropagation();
                    setWordingAt(at + (edgeOf(event) === "after" ? 1 : 0));
                  }}
                  onDragLeave={(event) => {
                    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
                    setWordingAt(null);
                  }}
                  onDrop={(event) => {
                    const insertAt = wordingAt;
                    setWordingAt(null);
                    const from = draggedWordingMove(event);
                    // A wording reorders inside its own entry only. The daemon takes the
                    // insertion index and settles the shift itself.
                    if (from === null || from.entry !== entry.index || insertAt === null) return;
                    event.preventDefault();
                    event.stopPropagation();
                    onMoveWording(section, entry.index, from.at, insertAt);
                  }}
                  className="group flex cursor-grab gap-1.5 py-1 pr-2 pl-9 active:cursor-grabbing"
                >
                  <span className="mt-1.5 ml-1 size-1 shrink-0 rounded-full bg-muted-foreground" />
                  <span className="flex-1 text-xs leading-snug">{highlight}</span>
                  <button
                    type="button"
                    aria-label="Save this wording to the bank"
                    title="Save this wording to the bank"
                    className="grid size-4 shrink-0 place-items-center rounded-sm text-muted-foreground opacity-0 hover:bg-muted hover:text-foreground group-hover:opacity-100 focus-visible:opacity-100"
                    onClick={(event) => {
                      event.stopPropagation();
                      onKeep(highlight);
                    }}
                  >
                    <BookmarkPlus className="size-3" />
                  </button>
                  <RemoveButton
                    label="Remove this wording"
                    onClick={() => onRemoveWording(section, entry.index, at)}
                  />
                </div>
              </div>
            ))}
            {wordingAt === entry.highlights.length && <DropLine />}
          </>
        )}
      </CollapsibleContent>
      {edge === "after" && <DropLine />}
    </Collapsible>
  );
}

export default function ResumeOutline() {
  const [sections, setSections] = useState<ResumeSection[] | null>(null);
  const [sectionTypes, setSectionTypes] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const [sectionEdge, setSectionEdge] = useState<
    { name: string; edge: "before" | "after" } | null
  >(null);
  const [keeping, setKeeping] = useState<string | null>(null);
  const load = useCallback(() => {
    // A section's type lives in the store, not the resume outline, which only knows what a
    // section is already holding — the bank is what says what it's allowed to hold. Loaded
    // together, not as two independent fetches: a drag landing between the two would find
    // sections rendered but no types to check it against yet, and drop unchecked.
    Promise.all([resumeOutline(), listSections()]).then(
      ([loadedSections, loadedSectionTypes]) => {
        setSections(loadedSections);
        setSectionTypes(
          Object.fromEntries(loadedSectionTypes.map((section) => [section.name, section.entry_type])),
        );
        setError(null);
      },
      (failure: Error) => setError(failure.message),
    );
  }, []);

  useEffect(load, [load]);

  useReloadOnHistory(load);

  const run = useCallback(
    (work: () => Promise<unknown>) =>
      resumeStep(work).then(load, (failure: Error) => setError(failure.message)),
    [load],
  );

  const dropEntry = useCallback(
    (section: string, entryId: number) => {
      run(async () => {
        const stored = await getEntry(entryId);
        if (!stored) throw new Error(`entry ${entryId} is no longer in the store`);
        return placeEntry(section, stored.entry_type, stored.fields);
      });
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
        run(() => placeSection(name));
      }}
    >
      {sections.length === 0 && (
        <Note>Drag a Section across to start this resume.</Note>
      )}
      <SaveToBank
        text={keeping ?? ""}
        open={keeping !== null}
        onOpenChange={(shown) => !shown && setKeeping(null)}
      />
      {sections.map((section, index) => {
        // rendercv requires every entry in a section to share one shape, so a section only ever
        // accepts the one entry type it was created with.
        const sectionType = sectionTypes[section.name];
        return (
          <div
            key={section.name}
            onDragOver={(event) => {
              if (!sectionType || !carriesEntryOfType(event, sectionType)) return;
              event.preventDefault();
              event.dataTransfer.dropEffect = "copy";
              setOver(section.name);
            }}
            onDragLeave={(event) => {
              // dragleave bubbles from the rows the pointer merely moved between.
              if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
              setOver(null);
            }}
            onDrop={(event) => {
              const dragged = draggedEntry(event);
              setOver(null);
              if (dragged === null || dragged.type !== sectionType) return;
              event.preventDefault();
              dropEntry(section.name, dragged.id);
            }}
          >
            <div
              className="group flex cursor-grab items-center gap-1.5 border-b px-2 py-1.5 active:cursor-grabbing"
              draggable
              onDragStart={(event) => {
                event.stopPropagation();
                startSectionMoveDrag(event, index);
              }}
              onDragOver={(event) => {
                if (!carriesSectionMove(event)) return;
                event.preventDefault();
                event.stopPropagation();
                event.dataTransfer.dropEffect = "move";
                setSectionEdge({ name: section.name, edge: edgeOf(event) });
              }}
              onDragLeave={(event) => {
                if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
                setSectionEdge(null);
              }}
              onDrop={(event) => {
                const heldEdge = sectionEdge;
                setSectionEdge(null);
                const from = draggedSectionMove(event);
                if (from === null || heldEdge?.name !== section.name) return;
                event.preventDefault();
                event.stopPropagation();
                const to = heldEdge.edge === "after" ? index + 1 : index;
                if (to !== from && to !== from + 1) run(() => moveResumeSection(from, to));
              }}
            >
              {sectionEdge?.name === section.name && sectionEdge.edge === "before" && <DropLine />}
              <span className="text-xs font-semibold">{section.name}</span>
              {sectionEdge?.name === section.name && sectionEdge.edge === "after" && <DropLine />}
            </div>
            {section.entries.map((entry) => (
              <EntryNode
                key={`${entry.index}-${JSON.stringify(entry.fields)}`}
                section={section.name}
                entry={entry}
                onPlaceWording={(s, i, text) => run(() => placeBullet(s, i, text))}
                onRemoveEntry={(s, i) => run(() => removeResumeEntry(s, i))}
                onRemoveWording={(s, i, at) => run(() => removeResumeBullet(s, i, at))}
                onSetField={(s, i, key, value) =>
                  run(() => setResumeEntryField(s, i, key, value))
                }
                onMoveEntry={(s, from, to) => run(() => moveResumeEntry(s, from, to))}
                onMoveWording={(s, i, from, to) =>
                  run(() => moveResumeBullet(s, i, from, to))
                }
                onKeep={setKeeping}
              />
            ))}
            {/* New entries always append, so the insertion point is always the bottom. */}
            {over === section.name && <DropLine />}
          </div>
        );
      })}
    </div>
  );
}
