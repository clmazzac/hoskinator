//! The HTTP daemon.
//!
//! Binds loopback only and ships no TLS (ADR-0003).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use hoskinator_core::home::{Home, HomeError};
use hoskinator_core::store::{Store, StoreError};
use jsonrpsee::RpcModule;

use crate::rpc::{
    ActiveRepository, ApplicationApi, ApplicationRpcServer, BulletApi, BulletRpcServer, EntryApi,
    EntryRpcServer, JobDescriptionApi, JobDescriptionRpcServer, ProfileApi, ProfileRpcServer,
    RenderApi, RenderRpcServer, RepositoryApi, RepositoryRpcServer, ResumeApi,
    ResumeRepositoryProvider, ResumeRpcServer, SearchApi, SearchRpcServer, SectionApi,
    SectionRpcServer, WorkspaceApi, WorkspaceRpcServer,
};
#[cfg(feature = "ai")]
use crate::rpc::{AiApi, AiRpcServer};

/// Port the daemon binds unless told otherwise.
pub const DEFAULT_PORT: u16 = 8737;
/// Path the rendered PDF is served from, so a browser can show what rendercv produced.
pub const PREVIEW_PATH: &str = "/preview.pdf";
/// Path the exported DOCX is served from.
pub const PREVIEW_DOCX_PATH: &str = "/preview.docx";

/// Where renders land: the platform temp directory, not the resume repository.
///
/// A render inside the repository would dirty `repository.status` on every keystroke of an
/// auto-render, and Home holds the store alone (`docs/decisions/home-and-config.md`).
pub fn preview_directory() -> PathBuf {
    std::env::temp_dir().join("hoskinator")
}

/// Path the JSON-RPC contract is served from.
const RPC_PATH: &str = "/rpc";

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("could not work out where Hoskinator keeps its data")]
    Home(#[from] HomeError),
    #[error("could not open the Master Store")]
    Store(#[from] StoreError),
    #[error("could not read Hoskinator configuration")]
    Config(#[from] hoskinator_core::config::ConfigError),
    #[error("could not build the JSON-RPC contract")]
    Contract(#[from] jsonrpsee::core::RegisterMethodError),
    #[error("could not bind {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("the daemon stopped unexpectedly")]
    Serve(#[source] std::io::Error),
}

/// Serves until the process is interrupted.
pub async fn run(port: u16) -> Result<(), ServeError> {
    let config = Home::config()?;
    let home = Home::resolve_with_config(&config)?;
    let store = Arc::new(Store::open(&home.store_path()).await?);
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| ServeError::Bind { address, source })?;

    println!("Hoskinator is listening on http://{address}{RPC_PATH}");
    println!("Store: {}", home.store_path().display());
    let default_repository_root = home.repositories_dir();
    axum::serve(
        listener,
        router(store, config.resume_repo, default_repository_root)?,
    )
    .with_graceful_shutdown(interrupted())
    .await
    .map_err(ServeError::Serve)
}

/// The daemon's routes, with every request passing the authenticator.
fn router(
    store: Arc<Store>,
    resume_repo: Option<PathBuf>,
    default_repository_root: PathBuf,
) -> Result<Router, ServeError> {
    // Shared so that switching repositories takes effect for every service immediately, rather
    // than only on the next start.
    let active = ActiveRepository::new(resume_repo);

    let mut module = RpcModule::new(());
    module.merge(ProfileApi::new(Arc::clone(&store), active.clone()).into_rpc())?;
    module.merge(SectionApi::new(Arc::clone(&store)).into_rpc())?;
    module.merge(EntryApi::new(Arc::clone(&store)).into_rpc())?;
    module.merge(BulletApi::new(Arc::clone(&store)).into_rpc())?;
    module.merge(SearchApi::new(Arc::clone(&store)).into_rpc())?;
    module.merge(JobDescriptionApi::new(Arc::clone(&store), active.clone()).into_rpc())?;
    module.merge(ResumeApi::new(Arc::clone(&store), active.clone()).into_rpc())?;
    module.merge(RenderApi::new(active.clone()).into_rpc())?;
    module.merge(ApplicationApi::new(Arc::clone(&store), active.clone()).into_rpc())?;
    module.merge(WorkspaceApi::new(active.clone(), default_repository_root).into_rpc())?;
    #[cfg(feature = "ai")]
    module.merge(AiApi::new(Arc::clone(&store), active.clone()).into_rpc())?;
    module.merge(RepositoryApi::new(ResumeRepositoryProvider::new(active)).into_rpc())?;

    Ok(Router::new()
        .route(RPC_PATH, post(dispatch))
        .route(PREVIEW_PATH, get(preview))
        .route(PREVIEW_DOCX_PATH, get(preview_docx))
        .fallback(crate::web::asset)
        .layer(axum::middleware::from_fn(authenticate))
        .with_state(Arc::new(module)))
}

/// Serves the most recent render. `?download=<name>` asks the browser to save it under that name.
async fn preview(axum::extract::Query(query): axum::extract::Query<PreviewQuery>) -> Response {
    served_file(PREVIEW_FILE, "application/pdf", query.download)
}

/// Serves the most recent DOCX export. `?download=<name>` asks the browser to save it under that name.
async fn preview_docx(axum::extract::Query(query): axum::extract::Query<PreviewQuery>) -> Response {
    served_file(
        PREVIEW_DOCX_FILE,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        query.download,
    )
}

/// Reads `file_name` out of the preview directory and serves it with `content_type`.
fn served_file(file_name: &str, content_type: &str, download: Option<String>) -> Response {
    let path = preview_directory().join(file_name);
    let Ok(bytes) = std::fs::read(&path) else {
        return (StatusCode::NOT_FOUND, "nothing has been rendered yet").into_response();
    };

    let disposition = match download.as_deref() {
        Some(name) if !name.is_empty() => {
            format!("attachment; filename=\"{}\"", name.replace('"', ""))
        }
        _ => "inline".to_string(),
    };

    (
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        bytes,
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct PreviewQuery {
    download: Option<String>,
}

/// The file every render writes, replaced each time.
pub const PREVIEW_FILE: &str = "preview.pdf";

/// The file every DOCX export writes, replaced each time.
pub const PREVIEW_DOCX_FILE: &str = "preview.docx";

/// Hands the request body to jsonrpsee and returns whatever it answers.
async fn dispatch(State(module): State<Arc<RpcModule<()>>>, body: String) -> Response {
    let Ok((answer, _)) = module.raw_json_request(&body, 1).await else {
        return (StatusCode::BAD_REQUEST, "malformed JSON-RPC request").into_response();
    };
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        answer.get().to_owned(),
    )
        .into_response()
}

async fn authenticate(request: Request, next: Next) -> Response {
    next.run(request).await
}

async fn interrupted() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use hoskinator_core::job_description::NewJobDescription;
    use hoskinator_core::profile::{OneOrMany, Profile};
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn test_router() -> (TempDir, Router) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .unwrap();
        let default_repository_root = dir.path().join("repositories");
        (
            dir,
            router(Arc::new(store), None, default_repository_root).unwrap(),
        )
    }

    async fn repository_router() -> (TempDir, Router) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .unwrap();
        let repo = dir.path().join("resume");
        let default_repository_root = dir.path().join("repositories");
        (
            dir,
            router(Arc::new(store), Some(repo), default_repository_root).unwrap(),
        )
    }

    /// A repository-backed router whose worktree already holds `resume.yaml`.
    async fn resume_router(yaml: &str) -> (TempDir, Router, PathBuf) {
        let (dir, router) = repository_router().await;
        let repo = dir.path().join("resume");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("resume.yaml"), yaml).unwrap();
        (dir, router, repo)
    }

    async fn call(router: Router, request: &str) -> serde_json::Value {
        let response = router
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(RPC_PATH)
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_owned()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn an_unknown_path_is_not_found() {
        let (_dir, router) = test_router().await;
        let response = router
            .oneshot(
                HttpRequest::builder()
                    .uri("/nothing-here")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn every_request_passes_through_the_authenticator() {
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&seen);
        let router = Router::new().layer(axum::middleware::from_fn(
            move |request: Request, next: Next| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    next.run(request).await
                }
            },
        ));
        router
            .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn profile_get_answers_an_unwritten_profile() {
        let (_dir, router) = test_router().await;
        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"profile.get","params":[]}"#,
        )
        .await;
        assert_eq!(
            answer["result"],
            serde_json::to_value(Profile::default()).unwrap()
        );
    }

    #[tokio::test]
    async fn a_profile_set_over_http_is_visible_to_profile_get() {
        let (_dir, router) = test_router().await;
        let profile = Profile {
            name: Some("Ada Lovelace".into()),
            email: Some(OneOrMany::One("ada@example.com".into())),
            ..Profile::default()
        };
        let params = serde_json::to_string(&profile).unwrap();
        let set = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"profile.set","params":[{params}]}}"#),
        )
        .await;
        assert!(set.get("error").is_none(), "set failed: {set}");
        let got = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"profile.get","params":[]}"#,
        )
        .await;
        assert_eq!(got["result"], serde_json::to_value(&profile).unwrap());
    }

    #[tokio::test]
    async fn setting_the_profile_refreshes_the_current_branch_s_resume_yaml() {
        let (_dir, router, repo) = resume_router("cv:\n  name: Old Name\n").await;

        let profile = Profile {
            name: Some("Ada Lovelace".into()),
            ..Profile::default()
        };
        let params = serde_json::to_string(&profile).unwrap();
        let set = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"profile.set","params":[{params}]}}"#),
        )
        .await;
        assert!(set.get("error").is_none(), "set failed: {set}");

        let written = std::fs::read_to_string(repo.join("resume.yaml")).unwrap();
        assert!(written.contains("Ada Lovelace"), "got: {written}");
        assert!(!written.contains("Old Name"), "got: {written}");
    }

    #[tokio::test]
    async fn setting_the_profile_without_a_resume_yaml_yet_still_succeeds() {
        let (dir, router) = repository_router().await;
        std::fs::create_dir_all(dir.path().join("resume")).unwrap();

        let profile = Profile {
            name: Some("Ada Lovelace".into()),
            ..Profile::default()
        };
        let params = serde_json::to_string(&profile).unwrap();
        let set = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"profile.set","params":[{params}]}}"#),
        )
        .await;
        assert!(set.get("error").is_none(), "set failed: {set}");
    }

    #[tokio::test]
    async fn a_created_section_is_visible_to_section_list() {
        let (_dir, router) = test_router().await;

        let created = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Experience","experience"]}"#,
        )
        .await;
        assert!(created.get("error").is_none(), "create failed: {created}");

        let listed = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"section.list","params":[]}"#,
        )
        .await;

        assert_eq!(
            listed["result"],
            serde_json::json!([{ "name": "Experience", "entry_type": "experience" }])
        );
    }

    #[tokio::test]
    async fn section_update_renames_and_retypes_in_one_call() {
        let (_dir, router) = test_router().await;
        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Writing","normal"]}"#,
        )
        .await;

        let updated = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"section.update","params":["Writing","Publications","publication"]}"#,
        )
        .await;

        assert_eq!(
            updated["result"],
            serde_json::json!({ "name": "Publications", "entry_type": "publication" })
        );
    }

    #[tokio::test]
    async fn a_deleted_section_leaves_the_list_empty() {
        let (_dir, router) = test_router().await;
        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Projects","normal"]}"#,
        )
        .await;

        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":2,"method":"section.delete","params":["Projects"]}"#,
        )
        .await;
        let listed = call(
            router,
            r#"{"jsonrpc":"2.0","id":3,"method":"section.list","params":[]}"#,
        )
        .await;

        assert_eq!(listed["result"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn a_duplicate_name_answers_section_invalid() {
        let (_dir, router) = test_router().await;
        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Projects","normal"]}"#,
        )
        .await;

        let second = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"section.create","params":["Projects","normal"]}"#,
        )
        .await;

        assert_eq!(second["error"]["code"], crate::rpc::SECTION_INVALID);
    }

    #[tokio::test]
    async fn deleting_an_absent_section_answers_section_not_found() {
        let (_dir, router) = test_router().await;

        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"section.delete","params":["Nowhere"]}"#,
        )
        .await;

        assert_eq!(answer["error"]["code"], crate::rpc::SECTION_NOT_FOUND);
    }

    #[tokio::test]
    async fn an_entry_type_rendercv_does_not_have_is_rejected() {
        let (_dir, router) = test_router().await;

        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Timeline","timeline"]}"#,
        )
        .await;

        assert!(answer.get("error").is_some(), "got {answer}");
    }

    #[tokio::test]
    async fn job_description_crud_and_search_work_over_http() {
        let (_dir, router) = test_router().await;
        let input = NewJobDescription {
            title: Some("Systems engineer".into()),
            text: "Build reliable Rust services.".into(),
        };
        let params = serde_json::to_string(&input).unwrap();

        let created = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"jd.create","params":[{params}]}}"#),
        )
        .await;
        let id = created["result"]["id"].as_i64().unwrap();
        assert_eq!(created["result"]["title"], "Systems engineer");
        assert_eq!(created["result"]["text"], "Build reliable Rust services.");
        assert!(created["result"]["created_at"].as_str().is_some());

        let listed = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":2,"method":"jd.list","params":["Rust"]}"#,
        )
        .await;
        assert_eq!(listed["result"].as_array().unwrap().len(), 1);
        assert_eq!(listed["result"][0]["id"], id);

        let got = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":3,"method":"jd.get","params":[{id}]}}"#),
        )
        .await;
        assert_eq!(got["result"], created["result"]);

        let deleted = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":4,"method":"jd.delete","params":[{id}]}}"#),
        )
        .await;
        assert_eq!(deleted["result"], true);

        let missing = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":5,"method":"jd.get","params":[{id}]}}"#),
        )
        .await;
        assert!(missing["result"].is_null());
    }

    #[tokio::test]
    async fn jd_match_scores_the_active_branch_s_resume_against_a_stored_jd() {
        let (dir, router) = repository_router().await;
        let repo = dir.path().join("resume");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("resume.yaml"), "cv:\n  name: Ada\nsections:\n  experience:\n    - highlights:\n        - Built services in Rust\n").unwrap();

        let jd = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"jd.create","params":[{"title":null,"text":"Looking for a Kubernetes engineer"}]}"#,
        )
        .await;
        let id = jd["result"]["id"].as_i64().unwrap();

        let matched = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"jd.match","params":[{id}]}}"#),
        )
        .await;
        assert!(matched.get("error").is_none(), "match failed: {matched}");
        assert_eq!(matched["result"]["score"], 0);
        assert_eq!(matched["result"]["missing"][0]["term"], "kubernetes");
    }

    #[tokio::test]
    async fn jd_match_against_an_absent_jd_answers_jd_not_found() {
        let (dir, router) = repository_router().await;
        std::fs::create_dir_all(dir.path().join("resume")).unwrap();

        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"jd.match","params":[404]}"#,
        )
        .await;
        assert_eq!(answer["error"]["code"], crate::rpc::JD_NOT_FOUND);
    }

    // Relies on the test process having no ANTHROPIC_API_KEY, same as CI (see .github/workflows).
    #[cfg(feature = "ai")]
    #[tokio::test]
    async fn ai_assess_without_a_configured_key_answers_ai_unconfigured() {
        let (dir, router) = repository_router().await;
        let repo = dir.path().join("resume");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("resume.yaml"), "cv:\n  name: Ada\n").unwrap();
        let jd = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"jd.create","params":[{"title":null,"text":"Anything"}]}"#,
        )
        .await;
        let id = jd["result"]["id"].as_i64().unwrap();

        let answer = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"ai.assess","params":[{id}]}}"#),
        )
        .await;
        assert_eq!(answer["error"]["code"], crate::rpc::AI_UNCONFIGURED);
    }

    #[tokio::test]
    async fn entry_crud_works_over_http() {
        let (_dir, router) = test_router().await;

        let created = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"entry.create","params":["experience",
               {"company":"Acme","position":"Engineer","start_date":"2021-06"}]}"#,
        )
        .await;
        let id = created["result"]["id"].as_i64().unwrap();
        assert_eq!(created["result"]["entry_type"], "experience");
        assert_eq!(created["result"]["fields"]["company"], "Acme");

        let got = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"entry.get","params":[{id}]}}"#),
        )
        .await;
        assert_eq!(got["result"], created["result"]);

        let listed = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":3,"method":"entry.list","params":["experience"]}"#,
        )
        .await;
        assert_eq!(listed["result"].as_array().unwrap().len(), 1);

        let updated = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":4,"method":"entry.update","params":[{id},
                   {{"company":"Acme","position":"Staff Engineer"}}]}}"#
            ),
        )
        .await;
        assert_eq!(updated["result"]["fields"]["position"], "Staff Engineer");
        assert!(updated["result"]["fields"]["start_date"].is_null());

        call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":5,"method":"entry.delete","params":[{id}]}}"#),
        )
        .await;

        let missing = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":6,"method":"entry.get","params":[{id}]}}"#),
        )
        .await;
        assert!(missing["result"].is_null());
    }

    #[tokio::test]
    async fn a_section_is_eligible_for_the_entries_of_its_type_over_http() {
        let (_dir, router) = test_router().await;
        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Education","education"]}"#,
        )
        .await;
        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":2,"method":"entry.create","params":["education",
               {"institution":"Cornell","area":"CS"}]}"#,
        )
        .await;
        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":3,"method":"entry.create","params":["bullet",{"bullet":"Shipped it."}]}"#,
        )
        .await;

        let eligible = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":4,"method":"entry.eligible","params":["Education"]}"#,
        )
        .await;
        assert_eq!(eligible["result"].as_array().unwrap().len(), 1);
        assert_eq!(eligible["result"][0]["fields"]["institution"], "Cornell");

        let missing = call(
            router,
            r#"{"jsonrpc":"2.0","id":5,"method":"entry.eligible","params":["Nowhere"]}"#,
        )
        .await;
        assert_eq!(missing["error"]["code"], crate::rpc::SECTION_NOT_FOUND);
    }

    #[tokio::test]
    async fn fields_the_entry_type_does_not_have_answer_entry_invalid() {
        let (_dir, router) = test_router().await;

        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"entry.create","params":["education",
               {"company":"Acme","position":"Engineer"}]}"#,
        )
        .await;

        assert_eq!(answer["error"]["code"], crate::rpc::ENTRY_INVALID);
    }

    #[tokio::test]
    async fn updating_an_absent_entry_answers_entry_not_found() {
        let (_dir, router) = test_router().await;

        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"entry.update","params":[404,{"bullet":"Shipped it."}]}"#,
        )
        .await;

        assert_eq!(answer["error"]["code"], crate::rpc::ENTRY_NOT_FOUND);
    }

    #[tokio::test]
    async fn braindump_is_settable_and_clearable_over_http() {
        let (_dir, router) = test_router().await;
        let created = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"entry.create","params":["experience",
               {"company":"Acme","position":"Engineer"}]}"#,
        )
        .await;
        let id = created["result"]["id"].as_i64().unwrap();
        assert!(created["result"]["braindump"].is_null());

        let set = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"entry.set_braindump","params":[{id},"  Cut latency in half after the Q2 rewrite.  "]}}"#
            ),
        )
        .await;
        assert_eq!(
            set["result"]["braindump"],
            "Cut latency in half after the Q2 rewrite."
        );

        let cleared = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"entry.set_braindump","params":[{id},"   "]}}"#
            ),
        )
        .await;
        assert!(cleared["result"]["braindump"].is_null());

        let missing = call(
            router,
            r#"{"jsonrpc":"2.0","id":4,"method":"entry.set_braindump","params":[404,"Notes."]}"#,
        )
        .await;
        assert_eq!(missing["error"]["code"], crate::rpc::ENTRY_NOT_FOUND);
    }

    /// Creates an entry of a type that carries bullets and answers with its id.
    async fn entry_with_bullets(router: &Router) -> i64 {
        let created = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"entry.create","params":["experience",
               {"company":"Acme","position":"Engineer"}]}"#,
        )
        .await;

        created["result"]["id"].as_i64().unwrap()
    }

    #[tokio::test]
    async fn bullet_and_variant_crud_works_over_http() {
        let (_dir, router) = test_router().await;
        let entry_id = entry_with_bullets(&router).await;

        let bullet = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"bullet.create","params":[{entry_id},
                   "Cut latency in half.","from the H1 review"]}}"#
            ),
        )
        .await;
        let bullet_id = bullet["result"]["id"].as_i64().unwrap();
        assert_eq!(bullet["result"]["variants"].as_array().unwrap().len(), 1);
        assert_eq!(bullet["result"]["variants"][0]["is_default"], true);
        assert_eq!(
            bullet["result"]["variants"][0]["note"],
            "from the H1 review"
        );

        let added = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"variant.create","params":[{bullet_id},
                   "Halved p99 latency.",null]}}"#
            ),
        )
        .await;
        let variant_id = added["result"]["id"].as_i64().unwrap();
        assert_eq!(added["result"]["is_default"], false);

        let promoted = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":4,"method":"variant.set_default","params":[{variant_id}]}}"#
            ),
        )
        .await;
        assert_eq!(promoted["result"]["is_default"], true);

        let listed = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":5,"method":"bullet.list","params":[{entry_id}]}}"#),
        )
        .await;
        let variants = listed["result"][0]["variants"].as_array().unwrap();
        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants.iter().filter(|v| v["is_default"] == true).count(),
            1
        );
        assert_eq!(variants[0]["id"], variant_id);

        let reworded = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":6,"method":"variant.update","params":[{variant_id},
                   "Halved tail latency.",null]}}"#
            ),
        )
        .await;
        assert_eq!(reworded["result"]["text"], "Halved tail latency.");

        call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":7,"method":"variant.delete","params":[{variant_id}]}}"#
            ),
        )
        .await;

        let after = call(
            router.clone(),
            &format!(r#"{{"jsonrpc":"2.0","id":8,"method":"bullet.get","params":[{bullet_id}]}}"#),
        )
        .await;
        assert_eq!(after["result"]["variants"].as_array().unwrap().len(), 1);
        assert_eq!(after["result"]["variants"][0]["is_default"], true);

        call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":9,"method":"bullet.delete","params":[{bullet_id}]}}"#
            ),
        )
        .await;

        let gone = call(
            router,
            &format!(r#"{{"jsonrpc":"2.0","id":10,"method":"bullet.get","params":[{bullet_id}]}}"#),
        )
        .await;
        assert!(gone["result"].is_null());
    }

    #[tokio::test]
    async fn deleting_the_last_variant_answers_bullet_invalid() {
        let (_dir, router) = test_router().await;
        let entry_id = entry_with_bullets(&router).await;
        let bullet = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"bullet.create","params":[{entry_id},"One.",null]}}"#
            ),
        )
        .await;
        let variant_id = bullet["result"]["variants"][0]["id"].as_i64().unwrap();

        let answer = call(
            router,
            &format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"variant.delete","params":[{variant_id}]}}"#
            ),
        )
        .await;

        assert_eq!(answer["error"]["code"], crate::rpc::BULLET_INVALID);
    }

    #[tokio::test]
    async fn a_bullet_on_a_type_without_highlights_answers_bullet_invalid() {
        let (_dir, router) = test_router().await;
        let created = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"entry.create","params":["bullet",{"bullet":"Shipped it."}]}"#,
        )
        .await;
        let entry_id = created["result"]["id"].as_i64().unwrap();

        let answer = call(
            router,
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"bullet.create","params":[{entry_id},"One.",null]}}"#
            ),
        )
        .await;

        assert_eq!(answer["error"]["code"], crate::rpc::BULLET_INVALID);
    }

    #[tokio::test]
    async fn an_absent_bullet_answers_bullet_not_found() {
        let (_dir, router) = test_router().await;

        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"bullet.delete","params":[404]}"#,
        )
        .await;

        assert_eq!(answer["error"]["code"], crate::rpc::BULLET_NOT_FOUND);
    }

    #[tokio::test]
    async fn search_finds_a_wording_and_answers_with_its_bullet() {
        let (_dir, router) = test_router().await;
        let entry_id = entry_with_bullets(&router).await;
        call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":2,"method":"bullet.create","params":[{entry_id},
                   "Cut p99 latency in half.",null]}}"#
            ),
        )
        .await;
        call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":3,"method":"bullet.create","params":[{entry_id},
                   "Rewrote the scheduler.",null]}}"#
            ),
        )
        .await;

        let found = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":4,"method":"search.query","params":["latency"]}"#,
        )
        .await;

        let hits = found["result"].as_array().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["kind"], "bullet");
        assert_eq!(hits[0]["entry"]["id"], entry_id);
        assert_eq!(
            hits[0]["matched_variant"]["text"],
            "Cut p99 latency in half."
        );
        assert_eq!(hits[0]["other_variants"], 0);

        let by_field = call(
            router,
            r#"{"jsonrpc":"2.0","id":5,"method":"search.query","params":["Acme"]}"#,
        )
        .await;

        assert_eq!(by_field["result"][0]["kind"], "entry");
        assert_eq!(by_field["result"][0]["entry"]["id"], entry_id);
    }

    #[tokio::test]
    async fn a_query_matching_nothing_answers_an_empty_list() {
        let (_dir, router) = test_router().await;

        let found = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"search.query","params":["underwater"]}"#,
        )
        .await;

        assert!(found["result"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn applications_are_unavailable_without_a_repository_with_a_github_remote() {
        let no_repository = call(
            test_router().await.1,
            r#"{"jsonrpc":"2.0","id":1,"method":"application.list","params":[]}"#,
        )
        .await;
        assert_eq!(
            no_repository["error"]["code"],
            crate::rpc::APPLICATION_UNAVAILABLE
        );

        // A repository exists on disk, but `git remote` was never set — there is nothing to
        // scope applications by.
        let (dir, router) = repository_router().await;
        git2::Repository::init(dir.path().join("resume")).unwrap();
        let no_remote = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"application.list","params":[]}"#,
        )
        .await;
        assert_eq!(
            no_remote["error"]["code"],
            crate::rpc::APPLICATION_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn applications_are_scoped_to_the_repository_s_github_remote() {
        let (dir, router) = repository_router().await;
        let repo_path = dir.path().join("resume");
        let repo = git2::Repository::init(&repo_path).unwrap();
        repo.remote("origin", "https://github.com/tester/resumes.git")
            .unwrap();

        let created = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"application.create","params":[
                {"company":"Acme","position":"Engineer","status":"draft","date_applied":null,
                 "listing_url":null,"resume_branch":null,"notes":null,"jd_text":null}]}"#,
        )
        .await;
        assert_eq!(created["result"]["company"], "Acme", "got {created}");

        let listed = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"application.list","params":[]}"#,
        )
        .await;
        assert_eq!(listed["result"].as_array().unwrap().len(), 1);
        assert_eq!(listed["result"][0]["company"], "Acme");
    }

    #[tokio::test]
    async fn repository_methods_are_available_over_json_rpc() {
        let (dir, router) = repository_router().await;
        let init = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"repository.init","params":[]}"#,
        )
        .await;
        assert!(init["result"].is_object());
        let unavailable = call(
            test_router().await.1,
            r#"{"jsonrpc":"2.0","id":2,"method":"repository.init","params":[]}"#,
        )
        .await;
        assert_eq!(unavailable["error"]["code"], -32004);

        let repo_path = dir.path().join("resume");
        std::fs::write(repo_path.join("resume.yaml"), "name: Ada\n").unwrap();
        let repo = git2::Repository::open(&repo_path).unwrap();
        repo.config().unwrap().set_str("user.name", "Ada").unwrap();
        repo.config()
            .unwrap()
            .set_str("user.email", "ada@example.com")
            .unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("resume.yaml")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();

        let branch = call(router.clone(), r#"{"jsonrpc":"2.0","id":3,"method":"repository.branch.create","params":[{"name":"revision"}]}"#).await;
        assert_eq!(branch["result"]["name"], "revision");
        let checkout = call(router.clone(), r#"{"jsonrpc":"2.0","id":4,"method":"repository.checkout","params":[{"branch":"revision"}]}"#).await;
        assert_eq!(checkout["result"]["head"]["branch"], "revision");
        std::fs::write(repo_path.join("resume.yaml"), "name: Grace\n").unwrap();
        let status = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":5,"method":"repository.status","params":[]}"#,
        )
        .await;
        assert!(status["result"]["entries"].is_array());
        let diff = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":6,"method":"repository.diff","params":[]}"#,
        )
        .await;
        assert!(diff["result"]["files"].is_array());
        let commit = call(router.clone(), r#"{"jsonrpc":"2.0","id":7,"method":"repository.commit","params":[{"message":"update"}]}"#).await;
        assert_eq!(commit["result"]["message"], "update");
        let log = call(
            router,
            r#"{"jsonrpc":"2.0","id":8,"method":"repository.log","params":[]}"#,
        )
        .await;
        assert!(log["result"]["commits"].is_array());
    }

    #[tokio::test]
    async fn resume_methods_are_available_over_json_rpc() {
        let (_dir, router, _repo) = resume_router("cv:\n  name: Old Name\n").await;

        let unavailable = call(
            test_router().await.1,
            r#"{"jsonrpc":"2.0","id":1,"method":"resume.read","params":[]}"#,
        )
        .await;
        assert_eq!(unavailable["error"]["code"], -32016);

        let read = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":2,"method":"resume.read","params":[]}"#,
        )
        .await;
        assert_eq!(read["result"], "cv:\n  name: Old Name\n");

        let set = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":3,"method":"profile.set","params":[{"name":"Ada Lovelace"}]}"#,
        )
        .await;
        assert!(set["result"].is_null());

        let edited = "cv:\n  name: Old Name # hand-edited\n  sections:\n    Experience: []\n";
        let write = call(
            router.clone(),
            &format!(
                r#"{{"jsonrpc":"2.0","id":4,"method":"resume.write","params":[{}]}}"#,
                serde_json::to_string(edited).unwrap()
            ),
        )
        .await;
        assert!(write["result"].is_null());

        let read_again = call(
            router,
            r#"{"jsonrpc":"2.0","id":5,"method":"resume.read","params":[]}"#,
        )
        .await;
        let written = read_again["result"].as_str().unwrap();
        assert!(written.contains("name: Ada Lovelace"));
        assert!(written.contains("# hand-edited"));
        assert!(written.contains("Experience: []"));
    }

    #[tokio::test]
    async fn render_answers_whether_it_can_run_without_erroring_over_a_missing_tool() {
        let (_dir, router) = test_router().await;

        let available = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"render.available","params":[]}"#,
        )
        .await;

        assert!(available["result"].is_boolean(), "got {available}");
    }

    #[tokio::test]
    async fn render_run_reports_what_is_missing_before_reaching_the_renderer() {
        let unconfigured = call(
            test_router().await.1,
            r#"{"jsonrpc":"2.0","id":1,"method":"render.run","params":["out","Resume"]}"#,
        )
        .await;
        assert_eq!(
            unconfigured["error"]["code"],
            crate::rpc::RENDER_UNAVAILABLE
        );

        let (dir, router) = repository_router().await;
        std::fs::create_dir_all(dir.path().join("resume")).unwrap();

        let unwritten = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"render.run","params":["out","Resume"]}"#,
        )
        .await;

        assert_eq!(unwritten["error"]["code"], crate::rpc::RENDER_NOT_FOUND);
    }

    #[tokio::test]
    async fn placing_an_entry_of_the_wrong_type_for_its_section_is_rejected() {
        let (_dir, router, repo) = resume_router("cv:\n  name: Ada\n").await;

        call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":1,"method":"section.create","params":["Experience","experience"]}"#,
        )
        .await;

        let mismatched = call(
            router.clone(),
            r#"{"jsonrpc":"2.0","id":2,"method":"resume.place_entry","params":[
                "Experience","education",{"institution":"asdf","area":"asdf"}]}"#,
        )
        .await;
        assert_eq!(
            mismatched["error"]["code"],
            crate::rpc::RESUME_SECTION_TYPE_MISMATCH,
            "got {mismatched}"
        );

        // Nothing was written: the section holds no entries at all yet.
        let written = std::fs::read_to_string(repo.join("resume.yaml")).unwrap();
        assert!(!written.contains("asdf"), "got: {written}");

        let matched = call(
            router,
            r#"{"jsonrpc":"2.0","id":3,"method":"resume.place_entry","params":[
                "Experience","experience",{"company":"Acme","position":"Engineer"}]}"#,
        )
        .await;
        assert!(matched.get("error").is_none(), "got {matched}");
        let written = std::fs::read_to_string(repo.join("resume.yaml")).unwrap();
        assert!(written.contains("Acme"), "got: {written}");
    }

    #[tokio::test]
    async fn placing_an_entry_into_a_section_that_does_not_exist_is_reported() {
        let (_dir, router, _repo) = resume_router("cv:\n  name: Ada\n").await;

        let missing = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"resume.place_entry","params":[
                "Nowhere","experience",{"company":"Acme","position":"Engineer"}]}"#,
        )
        .await;
        assert_eq!(missing["error"]["code"], crate::rpc::SECTION_NOT_FOUND);
    }

    #[tokio::test]
    async fn render_available_docx_answers_whether_it_can_run_without_erroring_over_a_missing_tool()
    {
        let (_dir, router) = test_router().await;

        let available = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"render.available_docx","params":[]}"#,
        )
        .await;

        assert!(available["result"].is_boolean(), "got {available}");
    }

    #[tokio::test]
    async fn render_docx_reports_what_is_missing_before_reaching_the_renderer() {
        let unconfigured = call(
            test_router().await.1,
            r#"{"jsonrpc":"2.0","id":1,"method":"render.docx","params":["out","Resume"]}"#,
        )
        .await;
        assert_eq!(
            unconfigured["error"]["code"],
            crate::rpc::RENDER_UNAVAILABLE
        );

        let (dir, router) = repository_router().await;
        std::fs::create_dir_all(dir.path().join("resume")).unwrap();

        let unwritten = call(
            router,
            r#"{"jsonrpc":"2.0","id":2,"method":"render.docx","params":["out","Resume"]}"#,
        )
        .await;

        assert_eq!(unwritten["error"]["code"], crate::rpc::RENDER_NOT_FOUND);
    }

    #[tokio::test]
    async fn an_unknown_method_is_reported_as_such() {
        let (_dir, router) = test_router().await;
        let answer = call(
            router,
            r#"{"jsonrpc":"2.0","id":1,"method":"profile.explode","params":[]}"#,
        )
        .await;
        assert_eq!(answer["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn a_malformed_request_is_a_bad_request() {
        let (_dir, router) = test_router().await;
        let response = router
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(RPC_PATH)
                    .header("content-type", "application/json")
                    .body(Body::from("{ not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
