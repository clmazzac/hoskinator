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
  if (!response.ok) throw new Error(`the daemon answered ${response.status}`);
  const answer = await response.json();
  if (answer.error) throw new RpcFailure(answer.error.code, answer.error.message);
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

/** An entry as the store holds it. `fields` is whatever rendercv reads for `entry_type`. */
export interface Entry {
  id: number;
  entry_type: string;
  fields: unknown;
  created_at: string;
}

export function createEntry(entryType: string, fields: unknown): Promise<Entry> {
  return call<Entry>("entry.create", [entryType, fields]);
}

export function getEntry(id: number): Promise<Entry | null> {
  return call<Entry | null>("entry.get", [id]);
}

export function listEntries(entryType: string | null = null): Promise<Entry[]> {
  return call<Entry[]>("entry.list", [entryType]);
}

export function eligibleEntries(section: string): Promise<Entry[]> {
  return call<Entry[]>("entry.eligible", [section]);
}

export function updateEntry(id: number, fields: unknown): Promise<Entry> {
  return call<Entry>("entry.update", [id, fields]);
}

export function deleteEntry(id: number): Promise<null> {
  return call<null>("entry.delete", [id]);
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

export interface Head { branch: string | null; commit_id: string | null }
export interface Branch { name: string; commit_id: string | null; is_head: boolean }
export interface RepositoryState { head: Head | null; branches: Branch[] }
export interface CreateBranchRequest { name: string }
export interface CheckoutRequest { branch: string }
export interface CommitRequest { message: string }
export interface CommitAuthor { name: string; email: string }
export interface GitTime { seconds_since_epoch: number; offset_minutes: number }
export interface CommitRecord { id: string; message: string; author: CommitAuthor; committed_at: GitTime; parents: string[] }
export type FileChange = "added" | "modified" | "deleted" | "renamed" | "typechange" | "conflicted" | "untracked";
export interface StatusEntry { path: string; old_path: string | null; index: FileChange | null; worktree: FileChange | null }
export interface RepositoryStatus { head: Head | null; entries: StatusEntry[] }
export type DiffLineKind = "context" | "addition" | "deletion";
export interface DiffLine { kind: DiffLineKind; old_line: number | null; new_line: number | null; content: string }
export interface DiffHunk { old_start: number; old_lines: number; new_start: number; new_lines: number; lines: DiffLine[] }
export interface FileDiff { old_path: string | null; new_path: string | null; status: FileChange; hunks: DiffHunk[] }
export interface RepositoryDiff { files: FileDiff[] }
export interface RepositoryLog { commits: CommitRecord[] }

export type RepositoryRequest =
  | { method: "repository.init"; params: [] }
  | { method: "repository.branch.create"; params: [CreateBranchRequest] }
  | { method: "repository.checkout"; params: [CheckoutRequest] }
  | { method: "repository.commit"; params: [CommitRequest] }
  | { method: "repository.status"; params: [] }
  | { method: "repository.diff"; params: [] }
  | { method: "repository.log"; params: [] };

export type RepositoryResult = RepositoryState | Branch | CommitRecord | RepositoryStatus | RepositoryDiff | RepositoryLog;

export function callRepository(request: RepositoryRequest): Promise<RepositoryResult> {
  return call<RepositoryResult>(request.method, request.params);
}
