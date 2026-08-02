//! The JSON-RPC contract.
//!
//! Method names and error codes here are the stable surface every frontend speaks (ADR-0003).

use std::sync::Arc;

use hoskinator_core::profile::Profile;
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

#[rpc(server)]
pub trait ProfileRpc {
    #[method(name = "profile.get")]
    async fn profile_get(&self) -> RpcResult<Profile>;

    #[method(name = "profile.set")]
    async fn profile_set(&self, profile: Profile) -> RpcResult<()>;
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

#[async_trait]
impl ProfileRpcServer for ProfileApi {
    async fn profile_get(&self) -> RpcResult<Profile> {
        self.store.profile().await.map_err(rpc_error)
    }

    async fn profile_set(&self, profile: Profile) -> RpcResult<()> {
        self.store.set_profile(&profile).await.map_err(rpc_error)
    }
}

/// Maps a [`StoreError`] onto the code its variant belongs to.
fn code_for(error: &StoreError) -> i32 {
    match error {
        StoreError::CreateDir { .. }
        | StoreError::Open { .. }
        | StoreError::Wal { .. }
        | StoreError::Migrate { .. }
        | StoreError::SchemaVersion(_) => STORE_UNAVAILABLE,
        StoreError::DecodeProfile { .. } => STORE_CORRUPT,
        StoreError::ReadProfile(_)
        | StoreError::WriteProfile(_)
        | StoreError::EncodeProfile { .. } => STORE_IO,
    }
}

/// Renders a [`StoreError`] as a JSON-RPC error, with its causes in the message.
fn rpc_error(error: StoreError) -> ErrorObjectOwned {
    let mut message = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }

    ErrorObjectOwned::owned(code_for(&error), message, None::<()>)
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

        assert_eq!(code_for(&error), STORE_UNAVAILABLE);
    }

    #[test]
    fn unreadable_stored_json_is_corruption() {
        let source = serde_json::from_str::<Profile>("not json").unwrap_err();
        let error = StoreError::DecodeProfile {
            column: "email",
            source,
        };

        assert_eq!(code_for(&error), STORE_CORRUPT);
    }

    #[test]
    fn every_code_sits_in_the_server_defined_range() {
        for code in [STORE_UNAVAILABLE, STORE_CORRUPT, STORE_IO] {
            assert!(
                (-32099..=-32000).contains(&code),
                "{code} is outside the JSON-RPC server-defined range"
            );
        }
    }

    #[test]
    fn the_codes_are_distinct() {
        let codes = [STORE_UNAVAILABLE, STORE_CORRUPT, STORE_IO];

        let unique: std::collections::HashSet<_> = codes.iter().collect();

        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn the_message_carries_the_underlying_cause() {
        let error = StoreError::CreateDir {
            path: "/nope".into(),
            source: io_error(),
        };

        let rendered = rpc_error(error);

        assert!(
            rendered.message().contains("disk on fire"),
            "got {:?}",
            rendered.message()
        );
    }
}
