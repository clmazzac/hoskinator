import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronRight, GripVertical, Sparkles } from "lucide-react";

import DeleteSectionPopover from "@/components/DeleteSectionPopover";
import EditableText from "@/components/EditableText";
import ProfileNode from "@/components/ProfileNode";
import RemoveButton from "@/components/RemoveButton";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  ENTRY_FIELDS,
  TEXT_FIELD,
  carriesBullets,
  entryLabel,
  isListField,
} from "@/entryFields";
import { type Async } from "@/lib/async";
import {
  joinElements,
  splitElements,
  startEntryDrag,
  startSectionDrag,
  startWordingDrag,
} from "@/lib/placement";
import { step, useReloadOnHistory } from "@/lib/history";
import { cn } from "@/lib/utils";
import {
  AI_UNCONFIGURED_CODE,
  BRAINDUMP_EMPTY_CODE,
  RpcFailure,
  createBullet,
  createSection,
  deleteSection,
  eligibleEntries,
  listBullets,
  listSections,
  setBraindump,
  suggestBullets,
  updateEntry,
  updateVariant,
  type Bullet,
  type DraftBullet,
  type Entry,
  type Section,
} from "@/rpc";

function Chevron({ open }: { open: boolean }) {
  return (
    <ChevronRight
      className={cn(
        "size-3.5 shrink-0 text-muted-foreground transition-transform",
        open && "rotate-90",
      )}
    />
  );
}

// The drag surface. Kept off the disclosure button, because a disabled button fires no drag
// events and a button that both drags and toggles swallows one of the two.
function Grip({ onDragStart }: { onDragStart: (event: React.DragEvent) => void }) {
  return (
    <span
      draggable
      onDragStart={onDragStart}
      className="grid size-4 shrink-0 cursor-grab place-items-center text-muted-foreground/40 hover:text-muted-foreground active:cursor-grabbing"
      title="Drag into the resume"
    >
      <GripVertical className="size-3" />
    </span>
  );
}

function Disclosure({
  open,
  onToggle,
  hidden,
}: {
  open: boolean;
  onToggle: () => void;
  hidden?: boolean;
}) {
  if (hidden) return <span className="size-3.5 shrink-0" />;
  return (
    <CollapsibleTrigger
      onClick={onToggle}
      className="grid size-3.5 shrink-0 place-items-center"
    >
      <Chevron open={open} />
    </CollapsibleTrigger>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="px-3 py-1.5 text-xs text-muted-foreground">{children}</p>;
}

function useLazy<T>(open: boolean, load: () => Promise<T>) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    load().then(setValue, (failure: Error) => setError(failure.message));
  }, [load]);

  useEffect(() => {
    if (!open || value !== null || error !== null) return;
    reload();
  }, [open, value, error, reload]);

  return { value, error, reload };
}

function BulletNode({ bullet, onEdited }: { bullet: Bullet; onEdited: () => void }) {
  const [open, setOpen] = useState(false);
  const shown =
    bullet.variants.find((variant) => variant.is_default) ?? bullet.variants[0];
  const others = bullet.variants.filter((variant) => variant !== shown);

  const edit = (
    id: number,
    text: string | null,
    note: string | null,
    was: { text: string; note: string | null },
  ) =>
    step(
      () => updateVariant(id, text, note),
      () => updateVariant(id, was.text, was.note),
    ).then(onEdited);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="flex items-start gap-1 py-1 pr-2 pl-7 hover:bg-muted/40">
        <Grip
          onDragStart={(event) => shown && startWordingDrag(event, shown.text)}
        />
        <Disclosure
          open={open}
          onToggle={() => setOpen(!open)}
          hidden={others.length === 0}
        />
        {shown && (
          <EditableText
            value={shown.text}
            onCommit={(text) => edit(shown.id, text, null, shown)}
            className="flex-1 text-xs leading-snug"
            multiline
          />
        )}
        {others.length > 0 && (
          <span className="mt-1 shrink-0 text-[10px] text-muted-foreground tabular-nums">
            {bullet.variants.length}
          </span>
        )}
      </div>

      <CollapsibleContent>
        {others.map((variant) => (
          <div
            key={variant.id}
            className="flex items-start gap-1 py-1 pr-2 pl-12 hover:bg-muted/40"
          >
            <Grip onDragStart={(event) => startWordingDrag(event, variant.text)} />
            <div className="flex-1">
              <EditableText
                value={variant.text}
                onCommit={(text) => edit(variant.id, text, null, variant)}
                className="text-xs leading-snug text-muted-foreground"
                multiline
              />
              <EditableText
                value={variant.note ?? ""}
                onCommit={(note) => edit(variant.id, null, note, variant)}
                placeholder="note"
                className="text-[10px] text-muted-foreground/70 italic"
              />
            </div>
          </div>
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

// A one-line entry keeps its elements in one comma-separated string. Each is shown as its own
// row so it drags and edits like a Bullet; all three gestures rewrite the string.
function ElementNode({
  entry,
  elements,
  index,
  onEdited,
}: {
  entry: Entry;
  elements: string[];
  index: number;
  onEdited: () => void;
}) {
  const held = (entry.fields ?? {}) as Record<string, unknown>;

  const rewrite = (next: string[]) =>
    step(
      () =>
        updateEntry(entry.id, {
          ...held,
          details: joinElements(next.filter(Boolean)),
        }),
      () => updateEntry(entry.id, held),
    ).then(onEdited);

  return (
    <div className="flex items-start gap-1 py-1 pr-2 pl-7 hover:bg-muted/40">
      <Grip onDragStart={(event) => startWordingDrag(event, elements[index])} />
      <span className="mt-1.5 ml-1 size-1 shrink-0 rounded-full bg-muted-foreground" />
      <EditableText
        value={elements[index]}
        onCommit={(next) =>
          rewrite(elements.map((held, at) => (at === index ? next : held)))
        }
        placeholder="(empty — commit to remove)"
        className="flex-1 text-xs leading-snug"
      />
    </div>
  );
}

// Every field the entry's type carries, editable. `entry.update` replaces the whole bag, so a
// commit merges the one changed key back into what is already there.
function EntryFields({ entry, onEdited }: { entry: Entry; onEdited: () => void }) {
  const held = (entry.fields ?? {}) as Record<string, unknown>;
  const names =
    entry.entry_type === "text" ? [TEXT_FIELD] : (ENTRY_FIELDS[entry.entry_type] ?? []);

  const commit = (name: string, next: string) => {
    const was = entry.fields;
    if (entry.entry_type === "text") {
      return step(
        () => updateEntry(entry.id, next),
        () => updateEntry(entry.id, was),
      ).then(onEdited);
    }
    const fields: Record<string, unknown> = { ...held };
    if (next.trim() === "") delete fields[name];
    else fields[name] = isListField(name) ? next.split("\n") : next;
    return step(
      () => updateEntry(entry.id, fields),
      () => updateEntry(entry.id, was),
    ).then(onEdited);
  };

  return (
    <div className="grid gap-0.5 py-1 pr-2 pl-12">
      {names
        .filter((name) => !(entry.entry_type === "one-line" && name === "details"))
        .map((name) => {
        const value = entry.entry_type === "text" ? entry.fields : held[name];
        const written = Array.isArray(value)
          ? value.join("\n")
          : value == null
            ? ""
            : String(value);
        return (
          <div key={name} className="flex items-baseline gap-2">
            <span className="w-20 shrink-0 text-right text-[10px] text-muted-foreground">
              {name.replace(/_/g, " ")}
            </span>
            <EditableText
              value={written}
              onCommit={(next) => commit(name, next)}
              placeholder="—"
              className="flex-1 text-xs"
            />
          </div>
        );
        })}
    </div>
  );
}

// A free-write scratchpad — never rendercv input, never in a resume.yaml.
function BraindumpEditor({ entry, onEdited }: { entry: Entry; onEdited: () => void }) {
  const commit = (next: string) => {
    const was = entry.braindump;
    return step(
      () => setBraindump(entry.id, next || null),
      () => setBraindump(entry.id, was),
    ).then(onEdited);
  };

  return (
    <div className="flex items-start gap-1 py-1 pr-2 pl-12">
      <EditableText
        value={entry.braindump ?? ""}
        onCommit={commit}
        placeholder="Notes — not shown on the resume."
        className="flex-1 text-xs leading-snug"
        multiline
      />
    </div>
  );
}

function suggestErrorMessage(error: unknown): string {
  if (error instanceof RpcFailure && error.code === AI_UNCONFIGURED_CODE) {
    return "AI isn't configured — set ANTHROPIC_API_KEY to enable this.";
  }
  if (error instanceof RpcFailure && error.code === BRAINDUMP_EMPTY_CODE) {
    return "Write some notes above first.";
  }
  return error instanceof Error ? error.message : "could not draft bullets";
}

function DraftBulletRow({ draft, onAdd }: { draft: DraftBullet; onAdd: () => void }) {
  const [added, setAdded] = useState(false);

  return (
    <div className="flex items-start gap-2 py-1">
      <div className="min-w-0 flex-1">
        <p className="text-xs leading-snug">{draft.text}</p>
        <p className="truncate text-[10px] text-muted-foreground italic">{draft.why}</p>
      </div>
      <Button
        variant="ghost"
        size="sm"
        disabled={added}
        onClick={() => {
          setAdded(true);
          onAdd();
        }}
        className="h-6 shrink-0 px-1.5 text-xs font-normal text-muted-foreground"
      >
        {added ? "Added" : "Add"}
      </Button>
    </div>
  );
}

// Hidden until there is a braindump to draft from.
function SuggestBullets({ entry, onAdded }: { entry: Entry; onAdded: () => void }) {
  const [state, setState] = useState<Async<DraftBullet[]> | null>(null);

  if (!entry.braindump) return null;

  const run = () => {
    setState({ status: "loading" });
    suggestBullets(entry.id).then(
      (drafts) => setState({ status: "ok", data: drafts }),
      (error: unknown) => setState({ status: "error", message: suggestErrorMessage(error) }),
    );
  };

  return (
    <div className="py-1 pr-2 pl-12">
      <Button
        variant="ghost"
        size="sm"
        onClick={run}
        disabled={state?.status === "loading"}
        className="h-6 w-fit gap-1 px-1.5 text-xs font-normal text-muted-foreground"
      >
        <Sparkles className="size-3" />
        Suggest bullets
      </Button>
      {state?.status === "loading" && (
        <p className="py-1 text-xs text-muted-foreground">Drafting…</p>
      )}
      {state?.status === "error" && (
        <p className="py-1 text-xs text-muted-foreground">{state.message}</p>
      )}
      {state?.status === "ok" && state.data.length === 0 && (
        <p className="py-1 text-xs text-muted-foreground">Nothing new to suggest.</p>
      )}
      {state?.status === "ok" && state.data.length > 0 && (
        <div className="flex flex-col divide-y">
          {state.data.map((draft, index) => (
            <DraftBulletRow
              key={index}
              draft={draft}
              onAdd={() => createBullet(entry.id, draft.text, null).then(onAdded)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function EntryNode({ entry, onEdited }: { entry: Entry; onEdited: () => void }) {
  const [open, setOpen] = useState(false);
  const { title, subtitle, dates } = entryLabel(entry.entry_type, entry.fields);
  const hasBullets = carriesBullets(entry.entry_type);
  const load = useCallback(() => listBullets(entry.id), [entry.id]);
  const { value: bullets, error, reload } = useLazy<Bullet[]>(open && hasBullets, load);
  const details = ((entry.fields ?? {}) as Record<string, unknown>).details;
  const elements =
    entry.entry_type === "one-line" && typeof details === "string"
      ? splitElements(details)
      : [];

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="flex items-center gap-1 py-1 pr-2 pl-4 hover:bg-muted/40">
        <Grip onDragStart={(event) => startEntryDrag(event, entry.id, entry.entry_type)} />
        <Disclosure open={open} onToggle={() => setOpen(!open)} />
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
      </div>

      <CollapsibleContent>
        <EntryFields entry={entry} onEdited={onEdited} />
        {hasBullets && <BraindumpEditor entry={entry} onEdited={onEdited} />}
        {hasBullets && <SuggestBullets entry={entry} onAdded={reload} />}
        {elements.map((_, index) => (
          <ElementNode
            key={index}
            entry={entry}
            elements={elements}
            index={index}
            onEdited={onEdited}
          />
        ))}
        {error && <Note>{error}</Note>}
        {hasBullets && bullets?.length === 0 && <Note>No bullets.</Note>}
        {bullets?.map((bullet) => (
          <BulletNode key={bullet.id} bullet={bullet} onEdited={reload} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

function SectionNode({ section, onDeleted }: { section: Section; onDeleted: () => void }) {
  const [open, setOpen] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const removeButtonRef = useRef<HTMLSpanElement>(null);
  const load = useCallback(() => eligibleEntries(section.name), [section.name]);
  const { value: entries, error, reload } = useLazy<Entry[]>(open, load);

  const remove = () => {
    setBusy(true);
    step(
      () => deleteSection(section.name),
      () => createSection(section.name, section.entry_type),
    ).then(() => {
      setBusy(false);
      setConfirming(false);
      onDeleted();
    });
  };

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="group flex items-center gap-1 border-b px-2 py-1.5 hover:bg-muted/40">
        <Grip onDragStart={(event) => startSectionDrag(event, section.name)} />
        <Disclosure open={open} onToggle={() => setOpen(!open)} />
        <span className="text-xs font-semibold">{section.name}</span>
        <span className="flex-1" />
        <span className="shrink-0 text-[10px] text-muted-foreground">
          {section.entry_type}
        </span>
        <span ref={removeButtonRef}>
          <RemoveButton label={`Delete ${section.name}`} onClick={() => setConfirming(true)} />
        </span>
      </div>

      <DeleteSectionPopover
        section={section.name}
        anchor={removeButtonRef}
        open={confirming}
        busy={busy}
        onOpenChange={setConfirming}
        onDelete={remove}
      />

      <CollapsibleContent className="border-b">
        {error && <Note>{error}</Note>}
        {entries?.length === 0 && <Note>No eligible entries.</Note>}
        {entries?.map((entry) => (
          <EntryNode key={entry.id} entry={entry} onEdited={reload} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}

export default function MasterStore() {
  const [sections, setSections] = useState<Section[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    listSections().then(setSections, (failure: Error) =>
      setError(failure.message),
    );
  }, []);

  useEffect(load, [load]);
  useReloadOnHistory(load);

  return (
    <div>
      <ProfileNode />
      {error && <Note>{error}</Note>}
      {!error && !sections && <Note>Loading…</Note>}
      {sections?.length === 0 && <Note>No Sections yet.</Note>}
      {sections?.map((section) => (
        <SectionNode key={section.name} section={section} onDeleted={load} />
      ))}
    </div>
  );
}
