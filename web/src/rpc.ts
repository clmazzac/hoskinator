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

/** A username on a network rendercv knows. */
export interface SocialConnection {
  network: string;
  username: string;
}

/** The singleton record mirroring rendercv's `cv:` header. */
export interface Profile {
  name: string | null;
  headline: string | null;
  location: string | null;
  photo: string | null;
  /** One value or several — the form the user wrote is kept. */
  email: string | string[] | null;
  phone: string | string[] | null;
  website: string | string[] | null;
  social_networks: SocialConnection[];
  custom_connections: unknown[];
}

export function getProfile(): Promise<Profile> {
  return call<Profile>("profile.get", []);
}

export function setProfile(profile: Profile): Promise<null> {
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

/** One wording of an accomplishment. */
export interface Variant {
  id: number;
  bullet_id: number;
  text: string;
  note: string | null;
  is_default: boolean;
}

/** One accomplishment inside an entry, with every wording it has. */
export interface Bullet {
  id: number;
  entry_id: number;
  position: number;
  variants: Variant[];
}

export function createBullet(
  entryId: number,
  text: string,
  note: string | null,
): Promise<Bullet> {
  return call<Bullet>("bullet.create", [entryId, text, note]);
}

export function listBullets(entryId: number): Promise<Bullet[]> {
  return call<Bullet[]>("bullet.list", [entryId]);
}

export function moveBullet(id: number, position: number): Promise<Bullet[]> {
  return call<Bullet[]>("bullet.move", [id, position]);
}

export function deleteBullet(id: number): Promise<null> {
  return call<null>("bullet.delete", [id]);
}

export function createVariant(
  bulletId: number,
  text: string,
  note: string | null,
): Promise<Variant> {
  return call<Variant>("variant.create", [bulletId, text, note]);
}

export function updateVariant(
  id: number,
  text: string | null,
  note: string | null,
): Promise<Variant> {
  return call<Variant>("variant.update", [id, text, note]);
}

export function setDefaultVariant(id: number): Promise<Variant> {
  return call<Variant>("variant.set_default", [id]);
}

export function deleteVariant(id: number): Promise<null> {
  return call<null>("variant.delete", [id]);
}

/** One thing a query matched. `entry` is the whole record, not a label. */
export type SearchHit =
  | { kind: "entry"; entry: unknown; rank: number }
  | {
      kind: "bullet";
      entry: unknown;
      bullet_id: number;
      matched_variant: Variant;
      other_variants: number;
      rank: number;
    };

export function search(query: string): Promise<SearchHit[]> {
  return call<SearchHit[]>("search.query", [query]);
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

/** One entry of a resume section, at the position it sits in the file. */
export interface ResumeEntry {
  index: number;
  fields: unknown;
  highlights: string[];
}

/** One section of a resume, named as the file names it. */
export interface ResumeSection {
  name: string;
  entries: ResumeEntry[];
}

export function readResume(): Promise<string> {
  return call<string>("resume.read", []);
}

export function resumeOutline(): Promise<ResumeSection[]> {
  return call<ResumeSection[]>("resume.outline", []);
}

export function placeBullet(
  section: string,
  entryIndex: number,
  text: string,
): Promise<null> {
  return call<null>("resume.place_bullet", [section, entryIndex, text]);
}

export function placeEntry(section: string, fields: unknown): Promise<null> {
  return call<null>("resume.place_entry", [section, fields]);
}

export function removeResumeEntry(
  section: string,
  entryIndex: number,
): Promise<null> {
  return call<null>("resume.remove_entry", [section, entryIndex]);
}

export function removeResumeBullet(
  section: string,
  entryIndex: number,
  highlightIndex: number,
): Promise<null> {
  return call<null>("resume.remove_bullet", [section, entryIndex, highlightIndex]);
}

export function setResumeEntryField(
  section: string,
  entryIndex: number,
  key: string,
  value: unknown,
): Promise<null> {
  return call<null>("resume.set_entry_field", [section, entryIndex, key, value]);
}

export function moveResumeEntry(
  section: string,
  from: number,
  to: number,
): Promise<null> {
  return call<null>("resume.move_entry", [section, from, to]);
}

export function moveResumeBullet(
  section: string,
  entryIndex: number,
  from: number,
  to: number,
): Promise<null> {
  return call<null>("resume.move_bullet", [section, entryIndex, from, to]);
}

export function placeSection(section: string): Promise<null> {
  return call<null>("resume.place_section", [section]);
}

export interface RenderedPdf {
  path: string;
}

export function renderAvailable(): Promise<boolean> {
  return call<boolean>("render.available", []);
}

export function renderPreview(): Promise<RenderedPdf> {
  return call<RenderedPdf>("render.preview", []);
}

export function resumeTheme(): Promise<string | null> {
  return call<string | null>("resume.theme", []);
}

export function resumeThemes(): Promise<string[]> {
  return call<string[]>("resume.themes", []);
}

export function setResumeTheme(theme: string): Promise<null> {
  return call<null>("resume.set_theme", [theme]);
}
