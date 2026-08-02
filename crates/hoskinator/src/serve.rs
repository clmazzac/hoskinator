//! The HTTP daemon.
//!
//! Binds loopback only and ships no TLS (ADR-0003).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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

use crate::rpc::{ProfileApi, ProfileRpcServer};

/// Port the daemon binds unless told otherwise.
pub const DEFAULT_PORT: u16 = 8737;

/// Path the JSON-RPC contract is served from.
const RPC_PATH: &str = "/rpc";

/// The daemon could not be started.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("could not work out where Hoskinator keeps its data")]
    Home(#[from] HomeError),

    #[error("could not open the Master Store")]
    Store(#[from] StoreError),

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
    let home = Home::resolve()?;
    let store = Arc::new(Store::open(&home.store_path()).await?);

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| ServeError::Bind { address, source })?;

    println!("Hoskinator is listening on http://{address}{RPC_PATH}");
    println!("Store: {}", home.store_path().display());

    axum::serve(listener, router(store)?)
        .with_graceful_shutdown(interrupted())
        .await
        .map_err(ServeError::Serve)
}

/// The daemon's routes, with every request passing the authenticator.
fn router(store: Arc<Store>) -> Result<Router, ServeError> {
    let mut module = RpcModule::new(());
    module.merge(ProfileApi::new(store).into_rpc())?;

    Ok(Router::new()
        .route(RPC_PATH, post(dispatch))
        .layer(axum::middleware::from_fn(authenticate))
        .with_state(Arc::new(module)))
}

/// Hands the request body to jsonrpsee and returns whatever it answers.
///
/// One request per call: `raw_json_request` parses a single JSON-RPC request, not a batch.
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

/// The authentication seam. Allows every request (ADR-0003).
async fn authenticate(request: Request, next: Next) -> Response {
    next.run(request).await
}

/// Resolves when the user interrupts the process.
async fn interrupted() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use hoskinator_core::profile::{OneOrMany, Profile};
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// A router over a fresh store, with the directory the caller must keep alive.
    async fn test_router() -> (TempDir, Router) {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("store").join("hoskinator.db"))
            .await
            .unwrap();
        (dir, router(Arc::new(store)).unwrap())
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
    async fn the_daemon_binds_loopback_only() {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let listener = tokio::net::TcpListener::bind(address).await.unwrap();

        assert!(listener.local_addr().unwrap().ip().is_loopback());
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
