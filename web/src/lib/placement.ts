// A drag's payload is unreadable until it drops — `dataTransfer.getData` only works then — but
// the *set* of MIME types present is readable throughout, including during dragover. Encoding an
// entry's type into its MIME type, rather than only in the payload, is what lets a section reject
// an incompatible drop before it lands: rendercv requires every entry in a section to share one
// shape, so a mismatched drop would otherwise write a resume.yaml that fails validation.

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

// What a drag of a whole Entry carries: the store id it came from, and its entry type — folded
// into the MIME type itself, one per type, rather than carried only in the payload.
const ENTRY_MIME_PREFIX = "application/x-hoskinator-entry-";

export function startEntryDrag(event: React.DragEvent, entryId: number, entryType: string): void {
  event.dataTransfer.setData(`${ENTRY_MIME_PREFIX}${entryType}`, String(entryId));
  event.dataTransfer.effectAllowed = "copy";
}

export function draggedEntry(event: React.DragEvent): { id: number; type: string } | null {
  const mime = event.dataTransfer.types.find((held) => held.startsWith(ENTRY_MIME_PREFIX));
  if (!mime) return null;
  const id = readNumber(event.dataTransfer.getData(mime));
  return id === null ? null : { id, type: mime.slice(ENTRY_MIME_PREFIX.length) };
}

/** Whether a drag in progress carries an Entry of any type. */
export function carriesEntry(event: React.DragEvent): boolean {
  return event.dataTransfer.types.some((held) => held.startsWith(ENTRY_MIME_PREFIX));
}

/** Whether a drag in progress carries an Entry of exactly this type — readable during dragover. */
export function carriesEntryOfType(event: React.DragEvent, entryType: string): boolean {
  return event.dataTransfer.types.includes(`${ENTRY_MIME_PREFIX}${entryType}`);
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

// What a drag of an Entry already in the resume carries: which entry of its section it is.
export const ENTRY_MOVE_MIME = "application/x-hoskinator-entry-move";

export function startEntryMoveDrag(event: React.DragEvent, from: number): void {
  event.dataTransfer.setData(ENTRY_MOVE_MIME, String(from));
  event.dataTransfer.effectAllowed = "move";
}

export function draggedEntryMove(event: React.DragEvent): number | null {
  return readNumber(event.dataTransfer.getData(ENTRY_MOVE_MIME));
}

export function carriesEntryMove(event: React.DragEvent): boolean {
  return event.dataTransfer.types.includes(ENTRY_MOVE_MIME);
}

/** A position inside one entry of a resume section. */
export interface Spot {
  entry: number;
  at: number;
}

// What a drag of a wording already in the resume carries: the entry it sits in, and where in it.
export const WORDING_MOVE_MIME = "application/x-hoskinator-wording-move";

export function startWordingMoveDrag(event: React.DragEvent, spot: Spot): void {
  event.dataTransfer.setData(WORDING_MOVE_MIME, writeSpot(spot));
  event.dataTransfer.effectAllowed = "move";
}

export function draggedWordingMove(event: React.DragEvent): Spot | null {
  return readSpot(event.dataTransfer.getData(WORDING_MOVE_MIME));
}

export function carriesWordingMove(event: React.DragEvent): boolean {
  return event.dataTransfer.types.includes(WORDING_MOVE_MIME);
}

// What a drag of one element of a one-line entry carries: the entry it sits in, and where in it.
// It travels as a wording as well.
export const ELEMENT_MIME = "application/x-hoskinator-element";

export function startElementDrag(
  event: React.DragEvent,
  spot: Spot,
  text: string,
): void {
  startWordingDrag(event, text);
  event.dataTransfer.setData(ELEMENT_MIME, writeSpot(spot));
}

export function draggedElement(event: React.DragEvent): Spot | null {
  return readSpot(event.dataTransfer.getData(ELEMENT_MIME));
}

export function carriesElement(event: React.DragEvent): boolean {
  return event.dataTransfer.types.includes(ELEMENT_MIME);
}

// What a drag of a whole Section carries: its name.
export const SECTION_MIME = "application/x-hoskinator-section";

export function startSectionDrag(event: React.DragEvent, name: string): void {
  event.dataTransfer.setData(SECTION_MIME, name);
  event.dataTransfer.effectAllowed = "copy";
}

export function draggedSection(event: React.DragEvent): string | null {
  return event.dataTransfer.getData(SECTION_MIME) || null;
}

export function carriesSection(event: React.DragEvent): boolean {
  return event.dataTransfer.types.includes(SECTION_MIME);
}

/** Reads a number a drag carried. An absent payload reads as absent, never as zero. */
function readNumber(held: string): number | null {
  if (held === "") return null;
  const value = Number(held);
  return Number.isInteger(value) ? value : null;
}

function writeSpot(spot: Spot): string {
  return `${spot.entry}:${spot.at}`;
}

function readSpot(held: string): Spot | null {
  const parts = held.split(":");
  if (parts.length !== 2) return null;
  const entry = readNumber(parts[0]);
  const at = readNumber(parts[1]);
  return entry === null || at === null ? null : { entry, at };
}
