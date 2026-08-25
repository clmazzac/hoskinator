// What a drag from the Master Store carries into the resume: one wording.
//
// A Bullet is an accomplishment; a Variant is one wording of it. Only the wording crosses, because
// the resume repo holds no reference back to the store (ADR-0001).

export const WORDING_MIME = "application/x-hoskinator-wording";

export function startWordingDrag(event: React.DragEvent, text: string): void {
  event.dataTransfer.setData(WORDING_MIME, text);
  event.dataTransfer.setData("text/plain", text);
  event.dataTransfer.effectAllowed = "copy";
}

export function draggedWording(event: React.DragEvent): string | null {
  return event.dataTransfer.getData(WORDING_MIME) || null;
}

/** Whether a drag in progress is carrying a wording. Read during dragover, where data is hidden. */
export function carriesWording(event: React.DragEvent): boolean {
  return event.dataTransfer.types.includes(WORDING_MIME);
}

// What a drag of a whole Entry carries: the store id it came from.
export const ENTRY_MIME = "application/x-hoskinator-entry";

export function startEntryDrag(event: React.DragEvent, entryId: number): void {
  event.dataTransfer.setData(ENTRY_MIME, String(entryId));
  event.dataTransfer.effectAllowed = "copy";
}

export function draggedEntry(event: React.DragEvent): number | null {
  const held = event.dataTransfer.getData(ENTRY_MIME);
  return held ? Number(held) : null;
}

export function carriesEntry(event: React.DragEvent): boolean {
  return event.dataTransfer.types.includes(ENTRY_MIME);
}

// A one-line entry holds its elements as a comma-separated string, so each element is addressed
// by its position in that list rather than by an index the file records.
export function splitElements(details: string): string[] {
  return details
    .split(",")
    .map((element) => element.trim())
    .filter(Boolean);
}

export function joinElements(elements: string[]): string {
  return elements.join(", ");
}
