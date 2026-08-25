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
