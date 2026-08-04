/** The daemon answered with a JSON-RPC error object. */
export class RpcFailure extends Error {
  readonly code: number;

  constructor(code: number, message: string) {
    super(message);
    this.name = "RpcFailure";
    this.code = code;
  }
}

export interface JobDescription {
  id: number;
  title: string | null;
  text: string;
  created_at: string;
}

export interface NewJobDescription {
  title?: string;
  text: string;
}

let nextId = 1;

async function call<T>(method: string, params: unknown[]): Promise<T> {
  const response = await fetch("/rpc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
  });

  if (!response.ok) {
    throw new Error(`the daemon answered ${response.status}`);
  }

  const answer = await response.json();
  if (answer.error) {
    throw new RpcFailure(answer.error.code, answer.error.message);
  }
  return answer.result as T;
}

export function getProfile(): Promise<unknown> {
  return call("profile.get", []);
}

export function setProfile(profile: unknown): Promise<null> {
  return call<null>("profile.set", [profile]);
}

/** A section as the store holds it. */
export interface Section {
  name: string;
  entry_type: string;
}

/** The nine entry types, in the order rendercv lists its arms. */
export const ENTRY_TYPES = [
  "text",
  "one-line",
  "normal",
  "experience",
  "education",
  "publication",
  "bullet",
  "numbered",
  "reversed-numbered",
] as const;

export function listSections(): Promise<Section[]> {
  return call<Section[]>("section.list", []);
}

export function createSection(name: string, entryType: string): Promise<Section> {
  return call<Section>("section.create", [name, entryType]);
}

export function renameSection(name: string, newName: string): Promise<Section> {
  return call<Section>("section.update", [name, newName, null]);
}

export function retypeSection(name: string, entryType: string): Promise<Section> {
  return call<Section>("section.update", [name, null, entryType]);
}

export function deleteSection(name: string): Promise<null> {
  return call<null>("section.delete", [name]);
}

export function createJobDescription(
  jobDescription: NewJobDescription,
): Promise<JobDescription> {
  return call("jd.create", [jobDescription]);
}

export function getJobDescription(id: number): Promise<JobDescription | null> {
  return call("jd.get", [id]);
}

export function listJobDescriptions(
  query: string | null = null,
): Promise<JobDescription[]> {
  return call("jd.list", [query]);
}

export function deleteJobDescription(id: number): Promise<boolean> {
  return call("jd.delete", [id]);
}
