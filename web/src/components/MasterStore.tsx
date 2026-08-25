import { useEffect, useState } from "react";
import { ChevronRight } from "lucide-react";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { carriesBullets, entryLabel } from "@/entryFields";
import { startEntryDrag, startWordingDrag } from "@/lib/placement";
import {
  eligibleEntries,
  listBullets,
  listSections,
  type Bullet,
  type Entry,
  type Section,
} from "@/rpc";
import { cn } from "@/lib/utils";

function Chevron({ className }: { className?: string }) {
  return (
    <ChevronRight
      className={cn(
        "size-3.5 shrink-0 text-muted-foreground transition-transform group-data-panel-open:rotate-90",
        className,
      )}
    />
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="px-3 py-1.5 text-xs text-muted-foreground">{children}</p>;
}

// Loads once, the first time its node is opened.
function useLazy<T>(open: boolean, load: () => Promise<T>) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || value !== null || error !== null) return;
    let live = true;
    load().then(
      (loaded) => live && setValue(loaded),
      (failure: Error) => live && setError(failure.message),
    );
    return () => {
      live = false;
    };
  }, [open]);

  return { value, error };
}

// A Bullet is one accomplishment. Its Variants are the wordings it has, one of
// them default. Git holds version history; the store holds no per-bullet history.
function BulletNode({ bullet }: { bullet: Bullet }) {
  const [open, setOpen] = useState(false);
  const shown =
    bullet.variants.find((variant) => variant.is_default) ?? bullet.variants[0];
  const others = bullet.variants.filter((variant) => variant !== shown);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger
        className="group flex w-full cursor-grab gap-1.5 py-1 pr-2 pl-9 text-left hover:bg-muted/50 active:cursor-grabbing"
        disabled={others.length === 0}
        draggable
        onDragStart={(event: React.DragEvent) =>
          shown && startWordingDrag(event, shown.text)
        }
      >
        {others.length > 0 ? (
          <Chevron className="mt-0.5" />
        ) : (
          <span className="mt-1.5 ml-1 size-1 shrink-0 rounded-full bg-muted-foreground" />
        )}
        <span className="flex-1 text-xs leading-snug">{shown?.text}</span>
        {others.length > 0 && (
          <span className="mt-0.5 shrink-0 text-[10px] text-muted-foreground tabular-nums">
            {bullet.variants.length}
          </span>
        )}
      </CollapsibleTrigger>

      <CollapsibleContent>
        {others.map((variant) => (
          <div
            key={variant.id}
            className="cursor-grab py-1 pr-2 pl-14 hover:bg-muted/50 active:cursor-grabbing"
            draggable
            onDragStart={(event: React.DragEvent) =>
              startWordingDrag(event, variant.text)
            }
          >
            <p className="text-xs leading-snug text-muted-foreground">
              {variant.text}
            </p>
            {variant.note && (
              <p className="mt-0.5 text-[10px] text-muted-foreground/70 italic">
                {variant.note}
              </p>
            )}
          </div>
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

function EntryNode({ entry }: { entry: Entry }) {
  const [open, setOpen] = useState(false);
  const { title, subtitle, dates } = entryLabel(entry.entry_type, entry.fields);
  const hasBullets = carriesBullets(entry.entry_type);
  const { value: bullets, error } = useLazy<Bullet[]>(
    open && hasBullets,
    () => listBullets(entry.id),
  );

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger
        className="group flex w-full cursor-grab items-baseline gap-1.5 py-1 pr-2 pl-6 text-left hover:bg-muted/50 active:cursor-grabbing"
        disabled={!hasBullets}
        draggable
        onDragStart={(event: React.DragEvent) => startEntryDrag(event, entry.id)}
      >
        {hasBullets ? <Chevron /> : <span className="size-3.5 shrink-0" />}
        <span className="truncate text-xs font-medium">{title}</span>
        {subtitle && (
          <span className="truncate text-xs text-muted-foreground">
            {subtitle}
          </span>
        )}
        <span className="flex-1" />
        {dates && (
          <span className="shrink-0 text-[10px] text-muted-foreground tabular-nums">
            {dates}
          </span>
        )}
      </CollapsibleTrigger>

      <CollapsibleContent>
        {error && <Note>{error}</Note>}
        {bullets?.length === 0 && <Note>No bullets.</Note>}
        {bullets?.map((bullet) => (
          <BulletNode key={bullet.id} bullet={bullet} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

function SectionNode({ section }: { section: Section }) {
  const [open, setOpen] = useState(false);
  const { value: entries, error } = useLazy<Entry[]>(open, () =>
    eligibleEntries(section.name),
  );

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="group flex w-full items-center gap-1.5 border-b px-2 py-1.5 text-left hover:bg-muted/50">
        <Chevron />
        <span className="text-xs font-semibold">{section.name}</span>
        <span className="flex-1" />
        <span className="shrink-0 text-[10px] text-muted-foreground">
          {section.entry_type}
        </span>
      </CollapsibleTrigger>

      <CollapsibleContent className="border-b">
        {error && <Note>{error}</Note>}
        {entries?.length === 0 && <Note>No eligible entries.</Note>}
        {entries?.map((entry) => (
          <EntryNode key={entry.id} entry={entry} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

export default function MasterStore() {
  const [sections, setSections] = useState<Section[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listSections().then(setSections, (failure: Error) =>
      setError(failure.message),
    );
  }, []);

  if (error) return <Note>{error}</Note>;
  if (!sections) return <Note>Loading…</Note>;
  if (sections.length === 0) return <Note>The Master Store is empty.</Note>;

  return (
    <div>
      {sections.map((section) => (
        <SectionNode key={section.name} section={section} />
      ))}
    </div>
  );
}
