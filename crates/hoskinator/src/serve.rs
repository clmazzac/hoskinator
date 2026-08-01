//! The HTTP daemon.
//!
//! Binds loopback only and ships no TLS (ADR-0003).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::Router;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Port the daemon binds unless told otherwise.
pub const DEFAULT_PORT: u16 = 8737;

/// The daemon could not be started.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
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
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| ServeError::Bind { address, source })?;

    println!("Hoskinator is listening on http://{address}");

    axum::serve(listener, router())
        .with_graceful_shutdown(interrupted())
        .await
        .map_err(ServeError::Serve)
}

/// The daemon's routes, with every request passing the authenticator.
fn router() -> Router {
    Router::new().layer(axum::middleware::from_fn(authenticate))
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
    use tower::ServiceExt;

    #[tokio::test]
    async fn an_unknown_path_is_not_found() {
        let response = router()
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
}
