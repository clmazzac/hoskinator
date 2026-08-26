// Undo and redo across both columns.
//
// A step is a pair of thunks. Resume edits invert by writing the document back as it was, because
// `resume.write` takes the whole text — one snapshot before and one after covers every placement,
// removal, and reorder without an inverse per operation. Store edits invert by restoring the value
// they replaced.

import { useEffect, useState } from "react";

import { readResume, writeResume } from "@/rpc";

export interface Step {
  undo: () => Promise<unknown>;
  redo: () => Promise<unknown>;
}

const past: Step[] = [];
const future: Step[] = [];
const listeners = new Set<() => void>();

/// Limits how far back the stack keeps documents, which are whole files.
const DEPTH = 50;

function announce() {
  for (const listener of listeners) listener();
}

export function push(step: Step): void {
  past.push(step);
  if (past.length > DEPTH) past.shift();
  future.length = 0;
  announce();
}

/// Runs a resume edit and records how to put the document back.
export async function resumeStep<T>(action: () => Promise<T>): Promise<T> {
  const before = await readResume();
  const result = await action();
  const after = await readResume();
  push({
    undo: () => writeResume(before),
    redo: () => writeResume(after),
  });
  return result;
}

/// Runs a store edit whose inverse the caller already knows.
export async function step<T>(
  action: () => Promise<T>,
  undo: () => Promise<unknown>,
): Promise<T> {
  const result = await action();
  push({ undo, redo: action });
  return result;
}

export async function undo(): Promise<void> {
  const held = past.pop();
  if (!held) return;
  future.push(held);
  announce();
  await held.undo();
  announce();
}

export async function redo(): Promise<void> {
  const held = future.pop();
  if (!held) return;
  past.push(held);
  announce();
  await held.redo();
  announce();
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

/// Runs `reload` whenever an undo or redo changes what is on disk.
export function useReloadOnHistory(reload: () => void): void {
  useEffect(() => {
    listeners.add(reload);
    return () => {
      listeners.delete(reload);
    };
  }, [reload]);
}
