// Undo and redo across both columns.
//
// A step is a pair of thunks. Resume edits invert by writing the document back as it was, because
// `resume.write` takes the whole text — one snapshot before and one after covers every placement,
// removal, and reorder without an inverse per operation. Store edits invert by restoring the value
// they replaced.

import { useEffect, useState } from "react";

import { readResume, writeResume } from "@/rpc";

/// What a step edited: the bank (Master Store) or the placed resume.
export type StepKind = "store" | "resume";

export interface Step {
  undo: () => Promise<unknown>;
  redo: () => Promise<unknown>;
  kind: StepKind;
}

const past: Step[] = [];
const future: Step[] = [];
const listeners = new Set<(kind: StepKind) => void>();

/// Limits how far back the stack keeps documents, which are whole files.
const DEPTH = 50;

function announce(kind: StepKind) {
  for (const listener of listeners) listener(kind);
}

export function push(step: Step): void {
  past.push(step);
  if (past.length > DEPTH) past.shift();
  future.length = 0;
  announce(step.kind);
}

/// Runs a resume edit and records how to put the document back.
export async function resumeStep<T>(action: () => Promise<T>): Promise<T> {
  const before = await readResume();
  const result = await action();
  const after = await readResume();
  push({
    undo: () => writeResume(before),
    redo: () => writeResume(after),
    kind: "resume",
  });
  return result;
}

/// Runs a store edit whose inverse the caller already knows.
export async function step<T>(
  action: () => Promise<T>,
  undo: () => Promise<unknown>,
): Promise<T> {
  const result = await action();
  push({ undo, redo: action, kind: "store" });
  return result;
}

export async function undo(): Promise<void> {
  const held = past.pop();
  if (!held) return;
  future.push(held);
  announce(held.kind);
  await held.undo();
  announce(held.kind);
}

export async function redo(): Promise<void> {
  const held = future.pop();
  if (!held) return;
  past.push(held);
  announce(held.kind);
  await held.redo();
  announce(held.kind);
}

/// Re-renders on every change to the stack, and after each undo or redo lands.
export function useHistory() {
  const [, bump] = useState(0);

  useEffect(() => {
    const listener = () => bump((count) => count + 1);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  return {
    canUndo: past.length > 0,
    canRedo: future.length > 0,
    undo,
    redo,
  };
}

/// Binds the usual shortcuts, unless the keystroke belongs to a field being edited.
export function useHistoryShortcuts(): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") return;
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || target?.isContentEditable) return;
      event.preventDefault();
      void (event.shiftKey ? redo() : undo());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

/// Runs `reload` whenever a step of `kind` lands, undoes, or redoes — everything by default.
export function useReloadOnHistory(reload: () => void, kind?: StepKind): void {
  useEffect(() => {
    const listener = (announced: StepKind) => {
      if (kind === undefined || announced === kind) reload();
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, [reload, kind]);
}
