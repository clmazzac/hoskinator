//! The JSON-RPC contract.
//!
//! Method names and error codes here are the stable surface every frontend speaks (ADR-0003).

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use hoskinator_core::application::{Application, NewApplication, STATUSES};
use hoskinator_core::bullet::{Bullet, BulletError, Variant};
use hoskinator_core::entry::{Entry, EntryError, EntryFields};
use hoskinator_core::job_description::{JobDescription, NewJobDescription};
use hoskinator_core::lineage::{self, Lineage};
use hoskinator_core::profile::Profile;
use hoskinator_core::render::{self, RenderError, RenderedDocx, RenderedPdf};
use hoskinator_core::repository::{
    Branch, CheckoutRequest, CommitRecord, CommitRequest, CreateBranchRequest, MergeOutcome,
    RepositoryDiff, RepositoryError, RepositoryLog, RepositoryState, RepositoryStatus,
    ResumeRepository,
};
use hoskinator_core::resume::{self, Design, ResumeError, ResumeSection};
use hoskinator_core::search::SearchHit;
use hoskinator_core::section::{EntryType, Section, SectionError};
use hoskinator_core::sheets::{self, SheetError};
use hoskinator_core::store::{Store, StoreError};
use hoskinator_core::tailoring::{self, MatchReport};
use hoskinator_core::workspace::{self, WorkspaceError, WorkspaceStatus};
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;

/// The store cannot be reached at all.
pub const STORE_UNAVAILABLE: i32 = -32001;
/// The store was reached but holds something unreadable.
pub const STORE_CORRUPT: i32 = -32002;
/// The store was reached but the read or write failed.
pub const STORE_IO: i32 = -32003;
/// No resume repository was configured or it could not be opened.
pub const REPOSITORY_UNAVAILABLE: i32 = -32004;
/// A requested repository branch or revision was not found.
pub const REPOSITORY_NOT_FOUND: i32 = -32005;
/// A repository operation violates a precondition or conflicts with current state.
pub const REPOSITORY_CONFLICT: i32 = -32006;
/// A repository request is invalid.
pub const REPOSITORY_INVALID_REQUEST: i32 = -32007;
/// Git identity configuration is unavailable.
pub const REPOSITORY_IDENTITY_UNAVAILABLE: i32 = -32008;
/// A Git operation failed.
pub const REPOSITORY_OPERATION: i32 = -32009;
/// The request described a section that cannot exist.
pub const SECTION_INVALID: i32 = -32010;
/// The request named a section that is not there.
pub const SECTION_NOT_FOUND: i32 = -32011;
/// The request described an entry that cannot exist.
pub const ENTRY_INVALID: i32 = -32012;
/// The request named an entry that is not there.
pub const ENTRY_NOT_FOUND: i32 = -32013;
/// The request described a bullet or variant that cannot exist.
pub const BULLET_INVALID: i32 = -32014;
/// The request named a bullet or variant that is not there.
pub const BULLET_NOT_FOUND: i32 = -32015;
/// No resume repository was configured.
pub const RESUME_UNAVAILABLE: i32 = -32016;
/// The current branch has no resume.yaml.
pub const RESUME_NOT_FOUND: i32 = -32017;
/// Reading, writing, or patching resume.yaml failed.
pub const RESUME_IO: i32 = -32018;
/// Writing resume.yaml would not validate against rendercv's schema.
pub const RESUME_INVALID: i32 = -32019;
/// The resume section has no entry at the index a placement named.
pub const RESUME_NO_SUCH_ENTRY: i32 = -32020;
/// The picker named a theme rendercv does not ship.
pub const RESUME_UNKNOWN_THEME: i32 = -32026;
/// No resume repository was configured to render.
pub const RENDER_UNAVAILABLE: i32 = -32021;
/// The current branch has no resume.yaml to render.
pub const RENDER_NOT_FOUND: i32 = -32022;
/// rendercv is not on PATH.
pub const RENDER_PROGRAM_MISSING: i32 = -32023;
/// rendercv ran and reported failure.
pub const RENDER_FAILED: i32 = -32024;
/// Running rendercv or collecting what it produced failed.
pub const RENDER_IO: i32 = -32025;
/// Reading or writing an application failed.
pub const APPLICATION_IO: i32 = -32027;
/// The GitHub CLI is missing or signed out.
pub const WORKSPACE_GITHUB: i32 = -32028;
/// Setting up the repository failed.
pub const WORKSPACE_SETUP: i32 = -32029;
/// pandoc is not on PATH.
pub const RENDER_PANDOC_MISSING: i32 = -32030;
/// pandoc ran and reported failure.
pub const RENDER_PANDOC_FAILED: i32 = -32031;
/// The linked Google Sheet could not be read, or none is linked.
pub const WORKSPACE_SHEET: i32 = -32032;
/// The entry's type does not match the section it was placed into.
pub const RESUME_SECTION_TYPE_MISMATCH: i32 = -32033;
/// No resume repository is configured, or it has no GitHub remote to scope applications by.
pub const APPLICATION_UNAVAILABLE: i32 = -32034;
/// The request named a Job Description that is not there.
pub const JD_NOT_FOUND: i32 = -32035;

#[rpc(server, client)]
pub trait ProfileRpc {
    #[method(name = "profile.get")]
    async fn profile_get(&self) -> RpcResult<Profile>;

    #[method(name = "profile.set")]
    async fn profile_set(&self, profile: Profile) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait SectionRpc {
    #[method(name = "section.create")]
    async fn section_create(&self, name: String, entry_type: EntryType) -> RpcResult<Section>;

    #[method(name = "section.list")]
    async fn section_list(&self) -> RpcResult<Vec<Section>>;

    #[method(name = "section.update")]
    async fn section_update(
        &self,
        name: String,
        new_name: Option<String>,
        entry_type: Option<EntryType>,
    ) -> RpcResult<Section>;

    #[method(name = "section.delete")]
    async fn section_delete(&self, name: String) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait EntryRpc {
    #[method(name = "entry.create")]
    async fn entry_create(
        &self,
        entry_type: EntryType,
        fields: serde_json::Value,
    ) -> RpcResult<Entry>;

    #[method(name = "entry.get")]
    async fn entry_get(&self, id: i64) -> RpcResult<Option<Entry>>;

    #[method(name = "entry.list")]
    async fn entry_list(&self, entry_type: Option<EntryType>) -> RpcResult<Vec<Entry>>;

    #[method(name = "entry.eligible")]
    async fn entry_eligible(&self, section: String) -> RpcResult<Vec<Entry>>;

    #[method(name = "entry.update")]
    async fn entry_update(&self, id: i64, fields: serde_json::Value) -> RpcResult<Entry>;

    #[method(name = "entry.delete")]
    async fn entry_delete(&self, id: i64) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait BulletRpc {
    #[method(name = "bullet.create")]
    async fn bullet_create(
        &self,
        entry_id: i64,
        text: String,
        note: Option<String>,
    ) -> RpcResult<Bullet>;

    #[method(name = "bullet.get")]
    async fn bullet_get(&self, id: i64) -> RpcResult<Option<Bullet>>;

    #[method(name = "bullet.list")]
    async fn bullet_list(&self, entry_id: i64) -> RpcResult<Vec<Bullet>>;

    #[method(name = "bullet.move")]
    async fn bullet_move(&self, id: i64, position: i32) -> RpcResult<Vec<Bullet>>;

    #[method(name = "bullet.delete")]
    async fn bullet_delete(&self, id: i64) -> RpcResult<()>;

    #[method(name = "variant.create")]
    async fn variant_create(
        &self,
        bullet_id: i64,
        text: String,
        note: Option<String>,
    ) -> RpcResult<Variant>;

    #[method(name = "variant.update")]
    async fn variant_update(
        &self,
        id: i64,
        text: Option<String>,
        note: Option<String>,
    ) -> RpcResult<Variant>;

    #[method(name = "variant.set_default")]
    async fn variant_set_default(&self, id: i64) -> RpcResult<Variant>;

    #[method(name = "variant.delete")]
    async fn variant_delete(&self, id: i64) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait SearchRpc {
    #[method(name = "search.query")]
    async fn search_query(&self, query: String) -> RpcResult<Vec<SearchHit>>;
}

#[rpc(server, client)]
pub trait JobDescriptionRpc {
    #[method(name = "jd.create")]
    async fn jd_create(&self, job_description: NewJobDescription) -> RpcResult<JobDescription>;

    #[method(name = "jd.get")]
    async fn jd_get(&self, id: i64) -> RpcResult<Option<JobDescription>>;

    #[method(name = "jd.list")]
    async fn jd_list(&self, query: Option<String>) -> RpcResult<Vec<JobDescription>>;

    #[method(name = "jd.delete")]
    async fn jd_delete(&self, id: i64) -> RpcResult<bool>;

    #[method(name = "jd.match")]
    async fn jd_match(&self, id: i64) -> RpcResult<MatchReport>;
}

#[rpc(server, client)]
pub trait RepositoryRpc {
    #[method(name = "repository.init")]
    async fn repository_init(&self) -> RpcResult<RepositoryState>;

    #[method(name = "repository.branch.create")]
    async fn repository_branch_create(&self, request: CreateBranchRequest) -> RpcResult<Branch>;

    #[method(name = "repository.branch.delete")]
    async fn repository_branch_delete(&self, name: String) -> RpcResult<RepositoryState>;

    #[method(name = "repository.checkout")]
    async fn repository_checkout(&self, request: CheckoutRequest) -> RpcResult<RepositoryState>;

    #[method(name = "repository.commit")]
    async fn repository_commit(&self, request: CommitRequest) -> RpcResult<CommitRecord>;

    #[method(name = "repository.status")]
    async fn repository_status(&self) -> RpcResult<RepositoryStatus>;

    #[method(name = "repository.diff")]
    async fn repository_diff(&self) -> RpcResult<RepositoryDiff>;

    #[method(name = "repository.log")]
    async fn repository_log(&self) -> RpcResult<RepositoryLog>;

    #[method(name = "repository.merge")]
    async fn repository_merge(&self, from: String) -> RpcResult<MergeOutcome>;

    /// Writes `contents` to `path` in the repository and stages it for the next commit.
    #[method(name = "repository.write_staged")]
    async fn repository_write_staged(&self, path: String, contents: String) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait ApplicationRpc {
    #[method(name = "application.list")]
    async fn application_list(&self) -> RpcResult<Vec<Application>>;

    #[method(name = "application.statuses")]
    async fn application_statuses(&self) -> RpcResult<Vec<String>>;

    #[method(name = "application.create")]
    async fn application_create(&self, application: NewApplication) -> RpcResult<Application>;

    #[method(name = "application.update")]
    async fn application_update(
        &self,
        id: i64,
        application: NewApplication,
    ) -> RpcResult<Application>;

    #[method(name = "application.delete")]
    async fn application_delete(&self, id: i64) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait WorkspaceRpc {
    #[method(name = "workspace.status")]
    async fn workspace_status(&self) -> RpcResult<WorkspaceStatus>;

    #[method(name = "workspace.repositories")]
    async fn workspace_repositories(&self) -> RpcResult<Vec<String>>;

    #[method(name = "workspace.create_github")]
    async fn workspace_create_github(
        &self,
        name: String,
        destination: PathBuf,
    ) -> RpcResult<WorkspaceStatus>;

    #[method(name = "workspace.connect")]
    async fn workspace_connect(
        &self,
        source: String,
        destination: PathBuf,
    ) -> RpcResult<WorkspaceStatus>;

    #[method(name = "workspace.lineage")]
    async fn workspace_lineage(&self, branch: String) -> RpcResult<Lineage>;

    #[method(name = "workspace.names")]
    async fn workspace_names(&self, slug: String, target: Option<String>) -> RpcResult<String>;

    #[method(name = "workspace.push")]
    async fn workspace_push(&self, branch: String) -> RpcResult<()>;

    /// Links a Google Sheet by its URL or bare id. The sheet must be shared "anyone with the
    /// link" as a viewer; nothing here authenticates.
    #[method(name = "workspace.link_sheet")]
    async fn workspace_link_sheet(&self, link: String) -> RpcResult<WorkspaceStatus>;

    /// Fetches the linked sheet's first tab as CSV.
    #[method(name = "workspace.sheet_csv")]
    async fn workspace_sheet_csv(&self) -> RpcResult<String>;
}

#[rpc(server, client)]
pub trait ResumeRpc {
    #[method(name = "resume.read")]
    async fn resume_read(&self) -> RpcResult<String>;

    #[method(name = "resume.write")]
    async fn resume_write(&self, text: String) -> RpcResult<()>;

    #[method(name = "resume.outline")]
    async fn resume_outline(&self) -> RpcResult<Vec<ResumeSection>>;

    #[method(name = "resume.place_bullet")]
    async fn resume_place_bullet(
        &self,
        section: String,
        entry_index: usize,
        text: String,
    ) -> RpcResult<()>;

    /// Rejects the placement if `entry_type` does not match the section's own configured type —
    /// rendercv's schema requires a section's entries to share one shape.
    #[method(name = "resume.place_entry")]
    async fn resume_place_entry(
        &self,
        section: String,
        entry_type: EntryType,
        fields: serde_json::Value,
    ) -> RpcResult<()>;

    #[method(name = "resume.design")]
    async fn resume_design(&self) -> RpcResult<Design>;

    #[method(name = "resume.set_top_note")]
    async fn resume_set_top_note(&self, show: bool) -> RpcResult<()>;

    #[method(name = "resume.themes")]
    async fn resume_themes(&self) -> RpcResult<Vec<String>>;

    #[method(name = "resume.set_theme")]
    async fn resume_set_theme(&self, theme: String) -> RpcResult<()>;

    #[method(name = "resume.place_section")]
    async fn resume_place_section(&self, section: String) -> RpcResult<()>;

    #[method(name = "resume.remove_entry")]
    async fn resume_remove_entry(&self, section: String, entry_index: usize) -> RpcResult<()>;

    #[method(name = "resume.move_entry")]
    async fn resume_move_entry(&self, section: String, from: usize, to: usize) -> RpcResult<()>;

    /// Moves the section at `from` so it lands before the section that sat at `to`.
    #[method(name = "resume.move_section")]
    async fn resume_move_section(&self, from: usize, to: usize) -> RpcResult<()>;

    #[method(name = "resume.move_bullet")]
    async fn resume_move_bullet(
        &self,
        section: String,
        entry_index: usize,
        from: usize,
        to: usize,
    ) -> RpcResult<()>;

    #[method(name = "resume.set_entry_field")]
    async fn resume_set_entry_field(
        &self,
        section: String,
        entry_index: usize,
        key: String,
        value: serde_json::Value,
    ) -> RpcResult<()>;

    #[method(name = "resume.remove_bullet")]
    async fn resume_remove_bullet(
        &self,
        section: String,
        entry_index: usize,
        highlight_index: usize,
    ) -> RpcResult<()>;
}

#[rpc(server, client)]
pub trait RenderRpc {
    #[method(name = "render.available")]
    async fn render_available(&self) -> RpcResult<bool>;

    #[method(name = "render.preview")]
    async fn render_preview(&self) -> RpcResult<RenderedPdf>;

    #[method(name = "render.run")]
    async fn render_run(&self, directory: PathBuf, file_name: String) -> RpcResult<RenderedPdf>;

    /// Whether a DOCX can be exported: both rendercv and pandoc must be on PATH.
    #[method(name = "render.available_docx")]
    async fn render_available_docx(&self) -> RpcResult<bool>;

    #[method(name = "render.preview_docx")]
    async fn render_preview_docx(&self) -> RpcResult<RenderedDocx>;

    #[method(name = "render.docx")]
    async fn render_docx(&self, directory: PathBuf, file_name: String) -> RpcResult<RenderedDocx>;
}

/// The resume repository currently in use, shared and read fresh by every RPC service.
#[derive(Clone)]
pub struct ActiveRepository(Arc<RwLock<Option<PathBuf>>>);

impl ActiveRepository {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self(Arc::new(RwLock::new(path)))
    }

    pub fn get(&self) -> Option<PathBuf> {
        self.0
            .read()
            .expect("active repository lock was poisoned")
            .clone()
    }

    pub fn set(&self, path: Option<PathBuf>) {
        *self.0.write().expect("active repository lock was poisoned") = path;
    }
}

/// Serves the Profile methods from one store.
pub struct ProfileApi {
    store: Arc<Store>,
    repository_path: ActiveRepository,
}

impl ProfileApi {
    pub fn new(store: Arc<Store>, repository_path: ActiveRepository) -> Self {
        Self {
            store,
            repository_path,
        }
    }
}

/// Declares a `pub struct $name` holding one store, with a matching `new`.
macro_rules! store_api {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        pub struct $name {
            store: Arc<Store>,
        }

        impl $name {
            pub fn new(store: Arc<Store>) -> Self {
                Self { store }
            }
        }
    };
}

store_api!(SectionApi, "Serves the Section methods from one store.");
store_api!(EntryApi, "Serves the Entry methods from one store.");
store_api!(
    BulletApi,
    "Serves the Bullet and Variant methods from one store."
);
store_api!(SearchApi, "Serves search from one store.");

/// Serves the Job Description methods from one store, scoring against the active repository's
/// resume.
pub struct JobDescriptionApi {
    store: Arc<Store>,
    repository_path: ActiveRepository,
}

impl JobDescriptionApi {
    pub fn new(store: Arc<Store>, repository_path: ActiveRepository) -> Self {
        Self {
            store,
            repository_path,
        }
    }
}

#[async_trait]
impl SectionRpcServer for SectionApi {
    async fn section_create(&self, name: String, entry_type: EntryType) -> RpcResult<Section> {
        self.store
            .create_section(&name, entry_type)
            .await
            .map_err(store_rpc_error)
    }

    async fn section_list(&self) -> RpcResult<Vec<Section>> {
        self.store.sections().await.map_err(store_rpc_error)
    }

    async fn section_update(
        &self,
        name: String,
        new_name: Option<String>,
        entry_type: Option<EntryType>,
    ) -> RpcResult<Section> {
        self.store
            .update_section(&name, new_name.as_deref(), entry_type)
            .await
            .map_err(store_rpc_error)
    }

    async fn section_delete(&self, name: String) -> RpcResult<()> {
        self.store
            .delete_section(&name)
            .await
            .map_err(store_rpc_error)
    }
}
#[async_trait]
impl ProfileRpcServer for ProfileApi {
    async fn profile_get(&self) -> RpcResult<Profile> {
        self.store.profile().await.map_err(store_rpc_error)
    }

    async fn profile_set(&self, profile: Profile) -> RpcResult<()> {
        self.store
            .set_profile(&profile)
            .await
            .map_err(store_rpc_error)?;

        // Best-effort: resume.yaml's cv: header carries a snapshot of the Profile, refreshed on
        // every resume write. Without this, that snapshot would only catch up on the next
        // placement, reorder, or other edit — not on the Profile edit itself.
        if let Some(path) = self.repository_path.get() {
            let _ = tokio::task::spawn_blocking(move || {
                resume::read(&path).and_then(|text| resume::write(&path, text, &profile))
            })
            .await;
        }

        Ok(())
    }
}

/// Lazily opens the configured repository for each repository RPC.
#[derive(Clone)]
pub struct ResumeRepositoryProvider {
    path: ActiveRepository,
}

impl ResumeRepositoryProvider {
    pub fn new(path: ActiveRepository) -> Self {
        Self { path }
    }

    fn open(&self) -> Result<ResumeRepository, RepositoryError> {
        let path = self
            .path
            .get()
            .ok_or(RepositoryError::MissingConfiguration)?;
        ResumeRepository::open_or_init(&path)
    }
}

/// Serves repository operations from the configured user-owned worktree.
pub struct RepositoryApi {
    provider: ResumeRepositoryProvider,
}

impl RepositoryApi {
    pub fn new(provider: ResumeRepositoryProvider) -> Self {
        Self { provider }
    }

    async fn operation<T, F>(&self, operation: F) -> RpcResult<T>
    where
        T: Send + 'static,
        F: FnOnce(ResumeRepository) -> Result<T, RepositoryError> + Send + 'static,
    {
        let provider = self.provider.clone();
        tokio::task::spawn_blocking(move || provider.open().and_then(operation))
            .await
            .map_err(|error| {
                ErrorObjectOwned::owned(REPOSITORY_OPERATION, error.to_string(), None::<()>)
            })?
            .map_err(repository_rpc_error)
    }
}

#[async_trait]
impl RepositoryRpcServer for RepositoryApi {
    async fn repository_init(&self) -> RpcResult<RepositoryState> {
        self.operation(|repository| repository.state()).await
    }

    async fn repository_branch_create(&self, request: CreateBranchRequest) -> RpcResult<Branch> {
        self.operation(move |repository| repository.create_branch(request))
            .await
    }

    async fn repository_branch_delete(&self, name: String) -> RpcResult<RepositoryState> {
        self.operation(move |repository| {
            repository.delete_branch(&name)?;
            repository.state()
        })
        .await
    }

    async fn repository_checkout(&self, request: CheckoutRequest) -> RpcResult<RepositoryState> {
        self.operation(move |repository| repository.checkout(request))
            .await
    }

    async fn repository_commit(&self, request: CommitRequest) -> RpcResult<CommitRecord> {
        self.operation(move |repository| repository.commit(request))
            .await
    }

    async fn repository_status(&self) -> RpcResult<RepositoryStatus> {
        self.operation(|repository| repository.status()).await
    }

    async fn repository_diff(&self) -> RpcResult<RepositoryDiff> {
        self.operation(|repository| repository.diff()).await
    }

    async fn repository_merge(&self, from: String) -> RpcResult<MergeOutcome> {
        self.operation(move |repository| repository.merge(&from))
            .await
    }

    async fn repository_log(&self) -> RpcResult<RepositoryLog> {
        self.operation(|repository| repository.log()).await
    }

    async fn repository_write_staged(&self, path: String, contents: String) -> RpcResult<()> {
        self.operation(move |repository| repository.write_staged(&path, &contents))
            .await
    }
}

/// Serves the resume.yaml methods from the configured user-owned worktree.
pub struct ResumeApi {
    store: Arc<Store>,
    repository_path: ActiveRepository,
}

impl ResumeApi {
    pub fn new(store: Arc<Store>, repository_path: ActiveRepository) -> Self {
        Self {
            store,
            repository_path,
        }
    }

    fn repository_path(&self) -> RpcResult<PathBuf> {
        self.repository_path
            .get()
            .ok_or_else(|| unavailable(RESUME_UNAVAILABLE))
    }

    async fn operation<T, F>(&self, operation: F) -> RpcResult<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, ResumeError> + Send + 'static,
    {
        tokio::task::spawn_blocking(operation)
            .await
            .map_err(|error| ErrorObjectOwned::owned(RESUME_IO, error.to_string(), None::<()>))?
            .map_err(resume_rpc_error)
    }
}

#[async_trait]
impl ResumeRpcServer for ResumeApi {
    async fn resume_read(&self) -> RpcResult<String> {
        let path = self.repository_path()?;
        self.operation(move || resume::read(&path)).await
    }

    async fn resume_write(&self, text: String) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::write(&path, text, &profile))
            .await
    }

    async fn resume_outline(&self) -> RpcResult<Vec<ResumeSection>> {
        let path = self.repository_path()?;
        self.operation(move || resume::outline(&path)).await
    }

    async fn resume_place_bullet(
        &self,
        section: String,
        entry_index: usize,
        text: String,
    ) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::place_bullet(&path, &section, entry_index, text, &profile))
            .await
    }

    async fn resume_place_entry(
        &self,
        section: String,
        entry_type: EntryType,
        fields: serde_json::Value,
    ) -> RpcResult<()> {
        let target = self
            .store
            .section(&section)
            .await
            .map_err(store_rpc_error)?
            .ok_or_else(|| section_not_found(&section))?;
        if target.entry_type != entry_type {
            return Err(section_type_mismatch(
                &section,
                target.entry_type,
                entry_type,
            ));
        }

        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::place_entry(&path, &section, fields, &profile))
            .await
    }

    async fn resume_design(&self) -> RpcResult<Design> {
        let path = self.repository_path()?;
        self.operation(move || resume::design(&path)).await
    }

    async fn resume_set_top_note(&self, show: bool) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::set_top_note(&path, show, &profile))
            .await
    }

    async fn resume_themes(&self) -> RpcResult<Vec<String>> {
        Ok(resume::THEMES.iter().map(|name| name.to_string()).collect())
    }

    async fn resume_set_theme(&self, theme: String) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::set_theme(&path, &theme, &profile))
            .await
    }

    async fn resume_place_section(&self, section: String) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::place_section(&path, &section, &profile))
            .await
    }

    async fn resume_remove_entry(&self, section: String, entry_index: usize) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::remove_entry(&path, &section, entry_index, &profile))
            .await
    }

    async fn resume_move_entry(&self, section: String, from: usize, to: usize) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || resume::move_entry(&path, &section, from, to, &profile))
            .await
    }

    async fn resume_move_section(&self, from: usize, to: usize) -> RpcResult<()> {
        let path = self.repository_path()?;
        self.operation(move || resume::move_section(&path, from, to))
            .await
    }

    async fn resume_move_bullet(
        &self,
        section: String,
        entry_index: usize,
        from: usize,
        to: usize,
    ) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || {
            resume::move_bullet(&path, &section, entry_index, from, to, &profile)
        })
        .await
    }

    async fn resume_set_entry_field(
        &self,
        section: String,
        entry_index: usize,
        key: String,
        value: serde_json::Value,
    ) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || {
            resume::set_entry_field(&path, &section, entry_index, &key, value, &profile)
        })
        .await
    }

    async fn resume_remove_bullet(
        &self,
        section: String,
        entry_index: usize,
        highlight_index: usize,
    ) -> RpcResult<()> {
        let path = self.repository_path()?;
        let profile = self.store.profile().await.map_err(store_rpc_error)?;
        self.operation(move || {
            resume::remove_bullet(&path, &section, entry_index, highlight_index, &profile)
        })
        .await
    }
}

/// Serves rendering from the configured user-owned worktree.
pub struct RenderApi {
    repository_path: ActiveRepository,
}

impl RenderApi {
    pub fn new(repository_path: ActiveRepository) -> Self {
        Self { repository_path }
    }
}

#[async_trait]
impl RenderRpcServer for RenderApi {
    async fn render_available(&self) -> RpcResult<bool> {
        blocking(render::is_available).await
    }

    async fn render_preview(&self) -> RpcResult<RenderedPdf> {
        self.render_run(
            crate::serve::preview_directory(),
            crate::serve::PREVIEW_FILE.to_string(),
        )
        .await
    }

    async fn render_run(&self, directory: PathBuf, file_name: String) -> RpcResult<RenderedPdf> {
        let repository = self
            .repository_path
            .get()
            .ok_or_else(|| unavailable(RENDER_UNAVAILABLE))?;

        blocking(move || render::pdf(&repository, &directory, &file_name))
            .await?
            .map_err(render_rpc_error)
    }

    async fn render_available_docx(&self) -> RpcResult<bool> {
        blocking(|| render::is_available() && render::is_pandoc_available()).await
    }

    async fn render_preview_docx(&self) -> RpcResult<RenderedDocx> {
        self.render_docx(
            crate::serve::preview_directory(),
            crate::serve::PREVIEW_DOCX_FILE.to_string(),
        )
        .await
    }

    async fn render_docx(&self, directory: PathBuf, file_name: String) -> RpcResult<RenderedDocx> {
        let repository = self
            .repository_path
            .get()
            .ok_or_else(|| unavailable(RENDER_UNAVAILABLE))?;

        blocking(move || render::docx(&repository, &directory, &file_name))
            .await?
            .map_err(render_rpc_error)
    }
}

/// Runs a rendercv call off the async runtime.
async fn blocking<T, F>(operation: F) -> RpcResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| ErrorObjectOwned::owned(RENDER_IO, error.to_string(), None::<()>))
}

#[async_trait]
impl EntryRpcServer for EntryApi {
    async fn entry_create(
        &self,
        entry_type: EntryType,
        fields: serde_json::Value,
    ) -> RpcResult<Entry> {
        let fields = EntryFields::parse(entry_type, fields)
            .map_err(|error| store_rpc_error(StoreError::Entry(error)))?;

        self.store
            .create_entry(&fields)
            .await
            .map_err(store_rpc_error)
    }

    async fn entry_get(&self, id: i64) -> RpcResult<Option<Entry>> {
        self.store.entry(id).await.map_err(store_rpc_error)
    }

    async fn entry_list(&self, entry_type: Option<EntryType>) -> RpcResult<Vec<Entry>> {
        self.store
            .entries(entry_type)
            .await
            .map_err(store_rpc_error)
    }

    async fn entry_eligible(&self, section: String) -> RpcResult<Vec<Entry>> {
        self.store
            .entries_for_section(&section)
            .await
            .map_err(store_rpc_error)
    }

    async fn entry_update(&self, id: i64, fields: serde_json::Value) -> RpcResult<Entry> {
        self.store
            .update_entry(id, fields)
            .await
            .map_err(store_rpc_error)
    }

    async fn entry_delete(&self, id: i64) -> RpcResult<()> {
        self.store.delete_entry(id).await.map_err(store_rpc_error)
    }
}

#[async_trait]
impl BulletRpcServer for BulletApi {
    async fn bullet_create(
        &self,
        entry_id: i64,
        text: String,
        note: Option<String>,
    ) -> RpcResult<Bullet> {
        self.store
            .create_bullet(entry_id, &text, note.as_deref())
            .await
            .map_err(store_rpc_error)
    }

    async fn bullet_get(&self, id: i64) -> RpcResult<Option<Bullet>> {
        self.store.bullet(id).await.map_err(store_rpc_error)
    }

    async fn bullet_list(&self, entry_id: i64) -> RpcResult<Vec<Bullet>> {
        self.store.bullets(entry_id).await.map_err(store_rpc_error)
    }

    async fn bullet_move(&self, id: i64, position: i32) -> RpcResult<Vec<Bullet>> {
        self.store
            .move_bullet(id, position)
            .await
            .map_err(store_rpc_error)
    }

    async fn bullet_delete(&self, id: i64) -> RpcResult<()> {
        self.store.delete_bullet(id).await.map_err(store_rpc_error)
    }

    async fn variant_create(
        &self,
        bullet_id: i64,
        text: String,
        note: Option<String>,
    ) -> RpcResult<Variant> {
        self.store
            .add_variant(bullet_id, &text, note.as_deref())
            .await
            .map_err(store_rpc_error)
    }

    async fn variant_update(
        &self,
        id: i64,
        text: Option<String>,
        note: Option<String>,
    ) -> RpcResult<Variant> {
        self.store
            .update_variant(id, text.as_deref(), note.as_deref())
            .await
            .map_err(store_rpc_error)
    }

    async fn variant_set_default(&self, id: i64) -> RpcResult<Variant> {
        self.store
            .set_default_variant(id)
            .await
            .map_err(store_rpc_error)
    }

    async fn variant_delete(&self, id: i64) -> RpcResult<()> {
        self.store.delete_variant(id).await.map_err(store_rpc_error)
    }
}

#[async_trait]
impl SearchRpcServer for SearchApi {
    async fn search_query(&self, query: String) -> RpcResult<Vec<SearchHit>> {
        self.store.search(&query).await.map_err(store_rpc_error)
    }
}

#[async_trait]
impl JobDescriptionRpcServer for JobDescriptionApi {
    async fn jd_create(&self, job_description: NewJobDescription) -> RpcResult<JobDescription> {
        self.store
            .create_job_description(&job_description)
            .await
            .map_err(store_rpc_error)
    }

    async fn jd_get(&self, id: i64) -> RpcResult<Option<JobDescription>> {
        self.store
            .job_description(id)
            .await
            .map_err(store_rpc_error)
    }

    async fn jd_list(&self, query: Option<String>) -> RpcResult<Vec<JobDescription>> {
        self.store
            .job_descriptions(query.as_deref())
            .await
            .map_err(store_rpc_error)
    }

    async fn jd_delete(&self, id: i64) -> RpcResult<bool> {
        self.store
            .delete_job_description(id)
            .await
            .map_err(store_rpc_error)
    }

    async fn jd_match(&self, id: i64) -> RpcResult<MatchReport> {
        let jd = self
            .store
            .job_description(id)
            .await
            .map_err(store_rpc_error)?
            .ok_or_else(|| jd_not_found(id))?;
        let path = self
            .repository_path
            .get()
            .ok_or_else(|| unavailable(RESUME_UNAVAILABLE))?;
        tokio::task::spawn_blocking(move || {
            resume::read(&path).map(|yaml| tailoring::match_report(&yaml, &jd.text))
        })
        .await
        .map_err(|error| ErrorObjectOwned::owned(RESUME_IO, error.to_string(), None::<()>))?
        .map_err(resume_rpc_error)
    }
}

/// Maps a [`StoreError`] onto the code its variant belongs to.
fn store_code_for(error: &StoreError) -> i32 {
    match error {
        StoreError::CreateDir { .. }
        | StoreError::PathEncoding { .. }
        | StoreError::Open { .. }
        | StoreError::Wal { .. }
        | StoreError::ForeignKeys { .. }
        | StoreError::Migrate { .. }
        | StoreError::SchemaVersion(_) => STORE_UNAVAILABLE,
        StoreError::DecodeProfile { .. } | StoreError::DecodeEntry { .. } => STORE_CORRUPT,
        StoreError::WriteApplication(_) | StoreError::ReadApplications(_) => APPLICATION_IO,
        StoreError::CreateJobDescription(_)
        | StoreError::ReadJobDescriptions(_)
        | StoreError::DeleteJobDescription(_)
        | StoreError::ReadProfile(_)
        | StoreError::WriteProfile(_)
        | StoreError::EncodeProfile { .. }
        | StoreError::ReadSection(_)
        | StoreError::WriteSection(_)
        | StoreError::ReadEntry(_)
        | StoreError::WriteEntry(_)
        | StoreError::EncodeEntry(_)
        | StoreError::ReadBullet(_)
        | StoreError::WriteBullet(_)
        | StoreError::ReadVariant(_)
        | StoreError::WriteVariant(_)
        | StoreError::Search(_) => STORE_IO,
        StoreError::Section(error) => match error {
            SectionError::BlankName
            | SectionError::DuplicateName { .. }
            | SectionError::UnknownEntryType(_) => SECTION_INVALID,
            SectionError::NoSuchSection { .. } => SECTION_NOT_FOUND,
        },
        StoreError::Entry(error) => match error {
            EntryError::Fields { .. } => ENTRY_INVALID,
            EntryError::NoSuchEntry { .. } => ENTRY_NOT_FOUND,
        },
        StoreError::Bullet(error) => match error {
            BulletError::EntryCarriesNoBullets { .. }
            | BulletError::BlankText
            | BulletError::LastVariant { .. } => BULLET_INVALID,
            BulletError::NoSuchBullet { .. } | BulletError::NoSuchVariant { .. } => {
                BULLET_NOT_FOUND
            }
        },
    }
}

fn repository_code_for(error: &RepositoryError) -> i32 {
    match error {
        RepositoryError::MissingConfiguration | RepositoryError::OpenOrInitialize { .. } => {
            REPOSITORY_UNAVAILABLE
        }
        RepositoryError::NotFound { .. } => REPOSITORY_NOT_FOUND,
        RepositoryError::Conflict { .. } => REPOSITORY_CONFLICT,
        RepositoryError::InvalidRequest { .. } => REPOSITORY_INVALID_REQUEST,
        RepositoryError::IdentityUnavailable(_) => REPOSITORY_IDENTITY_UNAVAILABLE,
        RepositoryError::Operation(_) => REPOSITORY_OPERATION,
        RepositoryError::Io { .. } => REPOSITORY_OPERATION,
    }
}

fn resume_code_for(error: &ResumeError) -> i32 {
    match error {
        ResumeError::NotFound { .. } => RESUME_NOT_FOUND,
        ResumeError::Invalid(_) => RESUME_INVALID,
        ResumeError::NoSuchEntry { .. } => RESUME_NO_SUCH_ENTRY,
        ResumeError::UnknownTheme { .. } => RESUME_UNKNOWN_THEME,
        ResumeError::Read { .. }
        | ResumeError::Write { .. }
        | ResumeError::Parse { .. }
        | ResumeError::EncodeProfile(_)
        | ResumeError::Patch { .. }
        | ResumeError::Decode { .. } => RESUME_IO,
    }
}

fn render_code_for(error: &RenderError) -> i32 {
    match error {
        RenderError::ResumeNotFound { .. } => RENDER_NOT_FOUND,
        RenderError::ProgramMissing => RENDER_PROGRAM_MISSING,
        RenderError::Failed { .. } => RENDER_FAILED,
        RenderError::Spawn(_) | RenderError::Scratch(_) | RenderError::NoOutput { .. } => RENDER_IO,
        RenderError::PandocMissing => RENDER_PANDOC_MISSING,
        RenderError::PandocFailed { .. } => RENDER_PANDOC_FAILED,
        RenderError::PandocSpawn(_)
        | RenderError::OutputDir { .. }
        | RenderError::NoDocxOutput { .. } => RENDER_IO,
    }
}

/// No resume repository is configured, whichever RPC needed one.
fn unavailable(code: i32) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code, "no resume repository is configured", None::<()>)
}

fn applications_unavailable() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        APPLICATION_UNAVAILABLE,
        "no resume repository is configured, or it has no GitHub remote yet",
        None::<()>,
    )
}

fn jd_not_found(id: i64) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        JD_NOT_FOUND,
        format!("no job description #{id}"),
        None::<()>,
    )
}

fn section_not_found(section: &str) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        SECTION_NOT_FOUND,
        format!("no section named {section}"),
        None::<()>,
    )
}

fn section_type_mismatch(section: &str, expected: EntryType, got: EntryType) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        RESUME_SECTION_TYPE_MISMATCH,
        format!(
            "{section} holds {expected} entries, not {got}",
            expected = expected.as_str(),
            got = got.as_str()
        ),
        None::<()>,
    )
}

fn source_message(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// Renders an error as a JSON-RPC error, with `code_for` choosing the code and its causes
/// making up the message.
fn rpc_error<E: std::error::Error>(code_for: fn(&E) -> i32, error: E) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(code_for(&error), source_message(&error), None::<()>)
}

/// Renders a [`StoreError`] as a JSON-RPC error, with its causes in the message.
fn store_rpc_error(error: StoreError) -> ErrorObjectOwned {
    rpc_error(store_code_for, error)
}

fn repository_rpc_error(error: RepositoryError) -> ErrorObjectOwned {
    rpc_error(repository_code_for, error)
}

fn resume_rpc_error(error: ResumeError) -> ErrorObjectOwned {
    rpc_error(resume_code_for, error)
}

fn render_rpc_error(error: RenderError) -> ErrorObjectOwned {
    rpc_error(render_code_for, error)
}

/// Serves the application tracker from one store.
pub struct ApplicationApi {
    store: Arc<Store>,
    repository_path: ActiveRepository,
}

impl ApplicationApi {
    pub fn new(store: Arc<Store>, repository_path: ActiveRepository) -> Self {
        Self {
            store,
            repository_path,
        }
    }

    /// The `owner/name` of the resume repository currently active, used to scope applications.
    async fn repository_scope(&self) -> RpcResult<String> {
        let path = self
            .repository_path
            .get()
            .ok_or_else(applications_unavailable)?;
        tokio::task::spawn_blocking(move || {
            workspace::remote(&path).and_then(|url| workspace::repository_slug(&url))
        })
        .await
        .map_err(|error| ErrorObjectOwned::owned(APPLICATION_IO, error.to_string(), None::<()>))?
        .ok_or_else(applications_unavailable)
    }
}

#[async_trait]
impl ApplicationRpcServer for ApplicationApi {
    async fn application_list(&self) -> RpcResult<Vec<Application>> {
        let repository = self.repository_scope().await?;
        self.store
            .applications(&repository)
            .await
            .map_err(store_rpc_error)
    }

    async fn application_statuses(&self) -> RpcResult<Vec<String>> {
        Ok(STATUSES.iter().map(|name| name.to_string()).collect())
    }

    async fn application_create(&self, application: NewApplication) -> RpcResult<Application> {
        let repository = self.repository_scope().await?;
        self.store
            .create_application(&application, &repository)
            .await
            .map_err(store_rpc_error)
    }

    async fn application_update(
        &self,
        id: i64,
        application: NewApplication,
    ) -> RpcResult<Application> {
        self.store
            .update_application(id, &application)
            .await
            .map_err(store_rpc_error)
    }

    async fn application_delete(&self, id: i64) -> RpcResult<()> {
        self.store
            .delete_application(id)
            .await
            .map_err(store_rpc_error)
    }
}

/// Serves repository setup, the GitHub account behind it, and the linked application sheet.
pub struct WorkspaceApi {
    repository_path: ActiveRepository,
    default_repository_root: PathBuf,
}

impl WorkspaceApi {
    pub fn new(repository_path: ActiveRepository, default_repository_root: PathBuf) -> Self {
        Self {
            repository_path,
            default_repository_root,
        }
    }

    /// Remembers a newly set-up repository, so the next start finds it.
    fn adopt(path: &Path) -> Result<(), WorkspaceError> {
        match hoskinator_core::home::config_file_path() {
            Some(config) => workspace::remember_repository(&config, path),
            None => Ok(()),
        }
    }

    /// Reads the linked sheet id fresh from disk.
    fn linked_sheet() -> Option<String> {
        hoskinator_core::home::Home::config()
            .ok()
            .and_then(|config| config.applications_sheet)
    }
}

#[async_trait]
impl WorkspaceRpcServer for WorkspaceApi {
    async fn workspace_status(&self) -> RpcResult<WorkspaceStatus> {
        let path = self.repository_path.get();
        let default_repository_root = self.default_repository_root.clone();
        workspace_blocking(move || {
            Ok(workspace::status(
                path.as_deref(),
                WorkspaceApi::linked_sheet().as_deref(),
                &default_repository_root,
            ))
        })
        .await
    }

    async fn workspace_repositories(&self) -> RpcResult<Vec<String>> {
        workspace_blocking(workspace::owned_repositories).await
    }

    async fn workspace_create_github(
        &self,
        name: String,
        destination: PathBuf,
    ) -> RpcResult<WorkspaceStatus> {
        let active = self.repository_path.clone();
        let default_repository_root = self.default_repository_root.clone();
        workspace_blocking(move || {
            let path = workspace::create_github(&name, &destination)?;
            WorkspaceApi::adopt(&path)?;
            active.set(Some(path.clone()));
            Ok(workspace::status(
                Some(&path),
                WorkspaceApi::linked_sheet().as_deref(),
                &default_repository_root,
            ))
        })
        .await
    }

    async fn workspace_connect(
        &self,
        source: String,
        destination: PathBuf,
    ) -> RpcResult<WorkspaceStatus> {
        let active = self.repository_path.clone();
        let default_repository_root = self.default_repository_root.clone();
        workspace_blocking(move || {
            let path = workspace::connect_github(&source, &destination)?;
            WorkspaceApi::adopt(&path)?;
            active.set(Some(path.clone()));
            Ok(workspace::status(
                Some(&path),
                WorkspaceApi::linked_sheet().as_deref(),
                &default_repository_root,
            ))
        })
        .await
    }

    async fn workspace_lineage(&self, branch: String) -> RpcResult<Lineage> {
        Ok(lineage::read(&branch))
    }

    async fn workspace_names(&self, slug: String, target: Option<String>) -> RpcResult<String> {
        Ok(match target {
            Some(target) => lineage::application_branch(&slug, &target),
            None => lineage::archetype_branch(&slug),
        })
    }

    async fn workspace_push(&self, branch: String) -> RpcResult<()> {
        let path = self
            .repository_path
            .get()
            .ok_or_else(|| unavailable(RESUME_UNAVAILABLE))?;
        workspace_blocking(move || workspace::push(&path, &branch)).await
    }

    async fn workspace_link_sheet(&self, link: String) -> RpcResult<WorkspaceStatus> {
        let path = self.repository_path.get();
        let default_repository_root = self.default_repository_root.clone();
        sheet_blocking(move || {
            let id = sheets::id_from(&link)?;
            if let Some(config) = hoskinator_core::home::config_file_path() {
                sheets::remember(&config, &id)?;
            }
            Ok(workspace::status(
                path.as_deref(),
                Some(&id),
                &default_repository_root,
            ))
        })
        .await
    }

    async fn workspace_sheet_csv(&self) -> RpcResult<String> {
        let id = WorkspaceApi::linked_sheet().ok_or(SheetError::NotLinked);
        sheet_blocking(move || sheets::csv(&id?)).await
    }
}

/// Runs a blocking sheet call and maps its failure onto a JSON-RPC error.
async fn sheet_blocking<T, F>(operation: F) -> RpcResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, SheetError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| ErrorObjectOwned::owned(WORKSPACE_SHEET, error.to_string(), None::<()>))?
        .map_err(|error| {
            ErrorObjectOwned::owned(WORKSPACE_SHEET, source_message(&error), None::<()>)
        })
}

/// Runs a blocking workspace call and maps its failure onto a JSON-RPC error.
async fn workspace_blocking<T, F>(operation: F) -> RpcResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, WorkspaceError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| ErrorObjectOwned::owned(WORKSPACE_SETUP, error.to_string(), None::<()>))?
        .map_err(workspace_rpc_error)
}

fn workspace_rpc_error(error: WorkspaceError) -> ErrorObjectOwned {
    let code = match error {
        WorkspaceError::GhMissing | WorkspaceError::GhSignedOut => WORKSPACE_GITHUB,
        _ => WORKSPACE_SETUP,
    };
    ErrorObjectOwned::owned(code, source_message(&error), None::<()>)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_error() -> std::io::Error {
        std::io::Error::other("disk on fire")
    }

    #[test]
    fn a_clone_of_active_repository_sees_a_later_set() {
        let active = ActiveRepository::new(Some(PathBuf::from("/old")));
        let clone = active.clone();

        active.set(Some(PathBuf::from("/new")));

        assert_eq!(clone.get(), Some(PathBuf::from("/new")));
    }

    #[test]
    fn an_unreachable_store_is_unavailable() {
        let error = StoreError::CreateDir {
            path: "/nope".into(),
            source: io_error(),
        };
        assert_eq!(store_code_for(&error), STORE_UNAVAILABLE);
    }

    #[test]
    fn unreadable_stored_json_is_corruption() {
        let source = serde_json::from_str::<Profile>("not json").unwrap_err();
        let error = StoreError::DecodeProfile {
            column: "email",
            source,
        };
        assert_eq!(store_code_for(&error), STORE_CORRUPT);
    }

    #[test]
    fn repository_errors_have_stable_codes() {
        assert_eq!(
            repository_code_for(&RepositoryError::MissingConfiguration),
            REPOSITORY_UNAVAILABLE
        );
        assert_eq!(
            repository_code_for(&RepositoryError::NotFound {
                name: "main".into()
            }),
            REPOSITORY_NOT_FOUND
        );
        assert_eq!(
            repository_code_for(&RepositoryError::Conflict {
                message: "x".into()
            }),
            REPOSITORY_CONFLICT
        );
        assert_eq!(
            repository_code_for(&RepositoryError::InvalidRequest {
                message: "x".into()
            }),
            REPOSITORY_INVALID_REQUEST
        );
        assert_eq!(
            repository_code_for(&RepositoryError::IdentityUnavailable(
                git2::Error::from_str("none")
            )),
            REPOSITORY_IDENTITY_UNAVAILABLE
        );
        assert_eq!(
            repository_code_for(&RepositoryError::Operation(git2::Error::from_str("nope"))),
            REPOSITORY_OPERATION
        );
    }

    #[test]
    fn resume_errors_have_stable_codes() {
        assert_eq!(
            resume_code_for(&ResumeError::NotFound { path: "x".into() }),
            RESUME_NOT_FOUND
        );
        assert_eq!(
            resume_code_for(&ResumeError::Invalid(vec!["bad".into()])),
            RESUME_INVALID
        );
        assert_eq!(
            resume_code_for(&ResumeError::Read {
                path: "x".into(),
                source: io_error(),
            }),
            RESUME_IO
        );
    }

    #[test]
    fn render_errors_have_stable_codes() {
        assert_eq!(
            render_code_for(&RenderError::ResumeNotFound { path: "x".into() }),
            RENDER_NOT_FOUND
        );
        assert_eq!(
            render_code_for(&RenderError::ProgramMissing),
            RENDER_PROGRAM_MISSING
        );
        assert_eq!(
            render_code_for(&RenderError::Failed {
                code: Some(1),
                diagnostics: "broke".into(),
            }),
            RENDER_FAILED
        );
        assert_eq!(render_code_for(&RenderError::Spawn(io_error())), RENDER_IO);
        assert_eq!(
            render_code_for(&RenderError::NoOutput { path: "x".into() }),
            RENDER_IO
        );
        assert_eq!(
            render_code_for(&RenderError::PandocMissing),
            RENDER_PANDOC_MISSING
        );
        assert_eq!(
            render_code_for(&RenderError::PandocFailed {
                code: Some(1),
                diagnostics: "broke".into(),
            }),
            RENDER_PANDOC_FAILED
        );
        assert_eq!(
            render_code_for(&RenderError::PandocSpawn(io_error())),
            RENDER_IO
        );
        assert_eq!(
            render_code_for(&RenderError::OutputDir {
                path: "x".into(),
                source: io_error(),
            }),
            RENDER_IO
        );
        assert_eq!(
            render_code_for(&RenderError::NoDocxOutput { path: "x".into() }),
            RENDER_IO
        );
    }

    #[test]
    fn a_render_failure_carries_what_the_renderer_said() {
        let error = RenderError::Failed {
            code: Some(1),
            diagnostics: "cv.sections.experience is not an entry type".into(),
        };

        assert!(
            render_rpc_error(error)
                .message()
                .contains("not an entry type")
        );
    }

    #[test]
    fn every_code_sits_in_the_server_defined_range_and_is_unique() {
        let codes = [
            STORE_UNAVAILABLE,
            STORE_CORRUPT,
            STORE_IO,
            REPOSITORY_UNAVAILABLE,
            REPOSITORY_NOT_FOUND,
            REPOSITORY_CONFLICT,
            REPOSITORY_INVALID_REQUEST,
            REPOSITORY_IDENTITY_UNAVAILABLE,
            REPOSITORY_OPERATION,
            SECTION_INVALID,
            SECTION_NOT_FOUND,
            ENTRY_INVALID,
            ENTRY_NOT_FOUND,
            BULLET_INVALID,
            BULLET_NOT_FOUND,
            RESUME_UNAVAILABLE,
            RESUME_NOT_FOUND,
            RESUME_IO,
            RESUME_INVALID,
            RENDER_UNAVAILABLE,
            RENDER_NOT_FOUND,
            RENDER_PROGRAM_MISSING,
            RENDER_FAILED,
            RENDER_IO,
            RENDER_PANDOC_MISSING,
            RENDER_PANDOC_FAILED,
            RESUME_SECTION_TYPE_MISMATCH,
            APPLICATION_UNAVAILABLE,
        ];
        assert!(codes.iter().all(|code| (-32099..=-32000).contains(code)));
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn the_message_carries_the_underlying_cause() {
        let error = StoreError::CreateDir {
            path: "/nope".into(),
            source: io_error(),
        };
        assert!(store_rpc_error(error).message().contains("disk on fire"));
    }
}
