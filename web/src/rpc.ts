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
  /** The application this posting was pasted onto, if it came from one rather than jd.create. */
  application_id: number | null;
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
  /** Free-write notes about this job or project — never rendercv input, never in a resume.yaml. */
  braindump: string | null;
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

export function setBraindump(id: number, text: string | null): Promise<Entry> {
  return call<Entry>("entry.set_braindump", [id, text]);
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

/** A candidate keyword extracted from a JD, and how much it counts for. */
export interface Keyword {
  term: string;
  weight: number;
}

export type WritingNoteKind = "unquantified" | "weak_opener";

/** A resume line that reads as a bullet, flagged for a specific writing issue. */
export interface WritingNote {
  line: string;
  kind: WritingNoteKind;
}

/** The deterministic half of the tailoring panel: keyword overlap only, no AI involved. */
export interface MatchReport {
  score: number;
  matched: Keyword[];
  missing: Keyword[];
  writing_notes: WritingNote[];
}

export function matchJobDescription(id: number): Promise<MatchReport> {
  return call("jd.match", [id]);
}

export interface Score {
  score: number;
  reason: string;
}

export interface Suggestion {
  on: string;
  suggestion: string;
  why: string;
}

/** Whether one of jd.match's missing keywords is covered by the resume some other way. */
export interface SemanticMatch {
  keyword: string;
  covered: boolean;
  evidence: string | null;
}

/**
 * The AI-judged half of the tailoring panel: relevance, tone, flow, semantic keyword coverage,
 * and rewrite suggestions.
 */
export interface Assessment {
  relevance: Score;
  tone: Score;
  flow: Score;
  semantic_coverage: SemanticMatch[];
  suggestions: Suggestion[];
}

/** Mirrors `rpc::AI_UNCONFIGURED` — the `ai` feature is built but no API key is set. */
export const AI_UNCONFIGURED_CODE = -32036;

/** Mirrors `rpc::BRAINDUMP_EMPTY` — the entry has no braindump to draft bullets from. */
export const BRAINDUMP_EMPTY_CODE = -32038;

export function assessResume(jdId: number): Promise<Assessment> {
  return call("ai.assess", [jdId]);
}

/** A candidate bullet, with the phrase in the braindump it is grounded in. */
export interface DraftBullet {
  text: string;
  why: string;
}

export function suggestBullets(entryId: number): Promise<DraftBullet[]> {
  return call("ai.suggest_bullets", [entryId]);
}

/** Whether a key is available, from Hoskinator's own settings or `ANTHROPIC_API_KEY`. */
export function aiStatus(): Promise<boolean> {
  return call("ai.status", []);
}

/** Writes or clears (`null`) the configured Anthropic key. Answers whether AI is now available. */
export function setAnthropicApiKey(key: string | null): Promise<boolean> {
  return call("ai.set_api_key", [key]);
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

export function writeResume(text: string): Promise<null> {
  return call<null>("resume.write", [text]);
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

export function placeEntry(section: string, entryType: string, fields: unknown): Promise<null> {
  return call<null>("resume.place_entry", [section, entryType, fields]);
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

export function moveResumeSection(from: number, to: number): Promise<null> {
  return call<null>("resume.move_section", [from, to]);
}

export function placeSection(section: string): Promise<null> {
  return call<null>("resume.place_section", [section]);
}

export function removeResumeSection(section: string): Promise<null> {
  return call<null>("resume.remove_section", [section]);
}

export interface RenderedPdf {
  path: string;
}

export interface RenderedDocx {
  path: string;
}

export function renderAvailable(): Promise<boolean> {
  return call<boolean>("render.available", []);
}

export function renderPreview(): Promise<RenderedPdf> {
  return call<RenderedPdf>("render.preview", []);
}

/** Whether a DOCX can be exported: both rendercv and pandoc must be on PATH. */
export function renderAvailableDocx(): Promise<boolean> {
  return call<boolean>("render.available_docx", []);
}

export function renderPreviewDocx(): Promise<RenderedDocx> {
  return call<RenderedDocx>("render.preview_docx", []);
}

/** Everything under `design:` the picker can set. */
export interface Design {
  theme: string | null;
  show_top_note: boolean;
}

export function resumeDesign(): Promise<Design> {
  return call<Design>("resume.design", []);
}

export function setTopNote(show: boolean): Promise<null> {
  return call<null>("resume.set_top_note", [show]);
}

export function resumeThemes(): Promise<string[]> {
  return call<string[]>("resume.themes", []);
}

export function setResumeTheme(theme: string): Promise<null> {
  return call<null>("resume.set_theme", [theme]);
}

// ---------------------------------------------------------------------------
// Repository, archetypes, and applications
// ---------------------------------------------------------------------------

export interface Branch {
  name: string;
  commit_id: string | null;
  is_head: boolean;
}

export interface RepositoryState {
  head: { branch: string | null; commit_id: string | null } | null;
  branches: Branch[];
}

export type Lineage =
  | { kind: "trunk" }
  | { kind: "archetype"; slug: string }
  | { kind: "application"; slug: string; target: string }
  | { kind: "loose" };

export interface WorkspaceStatus {
  gh_installed: boolean;
  github_login: string | null;
  repository_path: string | null;
  repository_ready: boolean;
  remote_url: string | null;
  applications_sheet: string | null;
  default_repository_root: string;
}

export interface MergeOutcome {
  branch: string;
  from: string;
  kind: string;
  commit_id: string | null;
}

export interface Application {
  id: number;
  company: string;
  position: string;
  status: string;
  date_applied: string | null;
  listing_url: string | null;
  resume_branch: string | null;
  notes: string | null;
  jd_text: string | null;
  created_at: string;
}

export type NewApplication = Omit<Application, "id" | "created_at">;

export function workspaceStatus(): Promise<WorkspaceStatus> {
  return call<WorkspaceStatus>("workspace.status", []);
}

export function ownedRepositories(): Promise<string[]> {
  return call<string[]>("workspace.repositories", []);
}

export function createGithubRepository(
  name: string,
  destination: string,
): Promise<WorkspaceStatus> {
  return call<WorkspaceStatus>("workspace.create_github", [name, destination]);
}

export function connectRepository(
  source: string,
  destination: string,
): Promise<WorkspaceStatus> {
  return call<WorkspaceStatus>("workspace.connect", [source, destination]);
}

/** Links a Google Sheet by its URL or bare id; it must be shared "anyone with the link" (viewer). */
export function linkSheet(link: string): Promise<WorkspaceStatus> {
  return call<WorkspaceStatus>("workspace.link_sheet", [link]);
}

/** Fetches the linked sheet's first tab as CSV. */
export function sheetCsv(): Promise<string> {
  return call<string>("workspace.sheet_csv", []);
}

export function pushBranch(branch: string): Promise<null> {
  return call<null>("workspace.push", [branch]);
}

export function branchName(slug: string, target: string | null): Promise<string> {
  return call<string>("workspace.names", [slug, target]);
}

export interface GoogleStatus {
  connected: boolean;
  account_email: string | null;
  sync_enabled: boolean;
  last_synced_at: number | null;
  last_sync_error: string | null;
}

export function googleStatus(): Promise<GoogleStatus> {
  return call<GoogleStatus>("google.status", []);
}

/** Stores the user's own Google Cloud OAuth client id and secret. */
export function setGoogleCredentials(
  clientId: string | null,
  clientSecret: string | null,
): Promise<boolean> {
  return call<boolean>("google.set_credentials", [clientId, clientSecret]);
}

/** Starts a connection: answers with the URL to open in a new tab. */
export function beginGoogleAuth(): Promise<string> {
  return call<string>("google.begin_auth", []);
}

export function disconnectGoogle(): Promise<boolean> {
  return call<boolean>("google.disconnect", []);
}

export interface SyncOutcome {
  pulled: number;
  created_locally: number;
  pushed_cells: number;
  appended_to_sheet: number;
}

/** Reconciles the linked sheet against the active repository's applications, right now. */
export function syncGoogleSheetNow(): Promise<SyncOutcome> {
  return call<SyncOutcome>("google.sync_now", []);
}

/** Starts or stops the background loop that reconciles the linked sheet every 30s. */
export function setGoogleSyncEnabled(enabled: boolean): Promise<boolean> {
  return call<boolean>("google.set_sync_enabled", [enabled]);
}

/**
 * Clears a deleted application's row from the linked sheet, so the next sync does not read it
 * back. A no-op if no account is connected or no sheet is linked.
 */
export function removeFromGoogleSheet(company: string, position: string): Promise<boolean> {
  return call<boolean>("google.remove_from_sheet", [company, position]);
}

export function repositoryState(): Promise<RepositoryState> {
  return call<RepositoryState>("repository.init", []);
}

export function createBranch(name: string, from: string | null): Promise<Branch> {
  return call<Branch>("repository.branch.create", [{ name, from }]);
}

export function checkoutBranch(branch: string): Promise<RepositoryState> {
  return call<RepositoryState>("repository.checkout", [{ branch }]);
}

export function deleteBranch(branch: string): Promise<RepositoryState> {
  return call<RepositoryState>("repository.branch.delete", [branch]);
}

export function commitResume(message: string): Promise<unknown> {
  return call("repository.commit", [{ message }]);
}

export function mergeBranch(from: string): Promise<MergeOutcome> {
  return call<MergeOutcome>("repository.merge", [from]);
}

export function repositoryStatus(): Promise<{ entries: unknown[] }> {
  return call<{ entries: unknown[] }>("repository.status", []);
}

/** Writes `contents` to `path` in the repository and stages it for the next commit. */
export function writeStagedFile(path: string, contents: string): Promise<null> {
  return call<null>("repository.write_staged", [path, contents]);
}

export function listApplications(): Promise<Application[]> {
  return call<Application[]>("application.list", []);
}

export function applicationStatuses(): Promise<string[]> {
  return call<string[]>("application.statuses", []);
}

export function createApplication(application: NewApplication): Promise<Application> {
  return call<Application>("application.create", [application]);
}

export function updateApplication(
  id: number,
  application: NewApplication,
): Promise<Application> {
  return call<Application>("application.update", [id, application]);
}

export function deleteApplication(id: number): Promise<null> {
  return call<null>("application.delete", [id]);
}
