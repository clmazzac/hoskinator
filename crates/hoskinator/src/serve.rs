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
use axum::routing::post;
use hoskinator_core::home::{Home, HomeError};
use hoskinator_core::store::{Store, StoreError};
use jsonrpsee::RpcModule;

use crate::rpc::{
    JobDescriptionApi, JobDescriptionRpcServer, ProfileApi, ProfileRpcServer, RepositoryApi,
    RepositoryRpcServer, ResumeRepositoryProvider, SectionApi, SectionRpcServer,
};

/// Port the daemon binds unless told otherwise.
pub const DEFAULT_PORT: u16 = 8737;
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
    axum::serve(listener, router(store, config.resume_repo)?)
        .with_graceful_shutdown(interrupted())
        .await
        .map_err(ServeError::Serve)
}

/// The daemon's routes, with every request passing the authenticator.
fn router(store: Arc<Store>, resume_repo: Option<PathBuf>) -> Result<Router, ServeError> {
    let mut module = RpcModule::new(());
    module.merge(ProfileApi::new(Arc::clone(&store)).into_rpc())?;
    module.merge(SectionApi::new(Arc::clone(&store)).into_rpc())?;
    module.merge(JobDescriptionApi::new(store).into_rpc())?;
    module.merge(RepositoryApi::new(ResumeRepositoryProvider::new(resume_repo)).into_rpc())?;

    Ok(Router::new()
        .route(RPC_PATH, post(dispatch))
        .fallback(crate::web::asset)
        .layer(axum::middleware::from_fn(authenticate))
        .with_state(Arc::new(module)))
}

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
        (dir, router(Arc::new(store), None).unwrap())
    }

    async fn repository_router() -> (TempDir, Router) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .unwrap();
        let repo = dir.path().join("resume");
        (dir, router(Arc::new(store), Some(repo)).unwrap())
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
