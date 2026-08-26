/// JSON-RPC against the running daemon, so a test can read the side effect of a drop.

const ENDPOINT = process.env.E2E_RPC_URL ?? "http://127.0.0.1:8737/rpc";

let nextId = 1;

export async function rpc<T>(method: string, params: unknown[] = []): Promise<T> {
  const response = await fetch(ENDPOINT, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
  });
  if (!response.ok) throw new Error(`the daemon answered ${response.status}`);
  const answer = await response.json();
  if (answer.error) throw new Error(answer.error.message);
  return answer.result as T;
}

export interface ResumeEntry {
  index: number;
  fields: Record<string, unknown> | null;
  highlights: string[];
}

export interface ResumeSection {
  name: string;
  entries: ResumeEntry[];
}

export interface StoreEntry {
  id: number;
  entry_type: string;
  fields: Record<string, unknown> | null;
}

export interface Variant {
  id: number;
  text: string;
  note: string | null;
  is_default: boolean;
}

export interface Bullet {
  id: number;
  entry_id: number;
  position: number;
  variants: Variant[];
}

export const outline = () => rpc<ResumeSection[]>("resume.outline");
export const readResume = () => rpc<string>("resume.read");
export const writeResume = (text: string) => rpc<null>("resume.write", [text]);
export const eligible = (section: string) =>
  rpc<StoreEntry[]>("entry.eligible", [section]);
export const bullets = (entryId: number) => rpc<Bullet[]>("bullet.list", [entryId]);

/// The one section of `outline` with this name.
export async function section(name: string): Promise<ResumeSection> {
  const found = (await outline()).find((held) => held.name === name);
  if (!found) throw new Error(`the resume has no ${name} section`);
  return found;
}

export async function entryTitles(name: string): Promise<string[]> {
  const held = await section(name);
  return held.entries.map(
    (entry) => String(entry.fields?.company ?? entry.fields?.label ?? ""),
  );
}

export async function details(name: string, index: number): Promise<string> {
  const held = await section(name);
  return String(held.entries[index].fields?.details ?? "");
}
