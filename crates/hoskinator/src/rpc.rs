//! The JSON-RPC contract.
//!
//! Method names and error codes here are the stable surface every frontend speaks (ADR-0003).

use std::path::PathBuf;
use std::sync::Arc;

use hoskinator_core::bullet::BulletError;
use hoskinator_core::entry::{Entry, EntryError, EntryFields};
use hoskinator_core::job_description::{JobDescription, NewJobDescription};
use hoskinator_core::profile::Profile;
use hoskinator_core::repository::{
    Branch, CheckoutRequest, CommitRecord, CommitRequest, CreateBranchRequest, RepositoryDiff,
    RepositoryError, RepositoryLog, RepositoryState, RepositoryStatus, ResumeRepository,
};
use hoskinator_core::section::{EntryType, Section, SectionError};
use hoskinator_core::store::{Store, StoreError};
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
pub trait JobDescriptionRpc {
    #[method(name = "jd.create")]
    async fn jd_create(&self, job_description: NewJobDescription) -> RpcResult<JobDescription>;

    #[method(name = "jd.get")]
    async fn jd_get(&self, id: i64) -> RpcResult<Option<JobDescription>>;

    #[method(name = "jd.list")]
    async fn jd_list(&self, query: Option<String>) -> RpcResult<Vec<JobDescription>>;

    #[method(name = "jd.delete")]
    async fn jd_delete(&self, id: i64) -> RpcResult<bool>;
}

#[rpc(server, client)]
pub trait RepositoryRpc {
    #[method(name = "repository.init")]
    async fn repository_init(&self) -> RpcResult<RepositoryState>;

    #[method(name = "repository.branch.create")]
    async fn repository_branch_create(&self, request: CreateBranchRequest) -> RpcResult<Branch>;

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
}

/// Serves the Profile methods from one store.
pub struct ProfileApi {
    store: Arc<Store>,
}

impl ProfileApi {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

/// Serves the Section methods from one store.
pub struct SectionApi {
    store: Arc<Store>,
}

impl SectionApi {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

/// Serves the Entry methods from one store.
pub struct EntryApi {
    store: Arc<Store>,
}

impl EntryApi {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

/// Serves the Job Description methods from one store.
pub struct JobDescriptionApi {
    store: Arc<Store>,
}

impl JobDescriptionApi {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
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
            .map_err(store_rpc_error)
    }
}

/// Lazily opens the configured repository for each repository RPC.
#[derive(Clone)]
pub struct ResumeRepositoryProvider {
    path: Option<PathBuf>,
}

impl ResumeRepositoryProvider {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    fn open(&self) -> Result<ResumeRepository, RepositoryError> {
        let path = self
            .path
            .as_deref()
            .ok_or(RepositoryError::MissingConfiguration)?;
        ResumeRepository::open_or_init(path)
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

    async fn repository_log(&self) -> RpcResult<RepositoryLog> {
        self.operation(|repository| repository.log()).await
    }
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
        | StoreError::WriteVariant(_) => STORE_IO,
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
    }
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

/// Renders a [`StoreError`] as a JSON-RPC error, with its causes in the message.
fn store_rpc_error(error: StoreError) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(store_code_for(&error), source_message(&error), None::<()>)
}

fn repository_rpc_error(error: RepositoryError) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        repository_code_for(&error),
        source_message(&error),
        None::<()>,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_error() -> std::io::Error {
        std::io::Error::other("disk on fire")
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
