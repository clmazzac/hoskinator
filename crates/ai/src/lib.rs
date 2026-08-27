//! Hoskinator's optional AI layer.
//!
//! Depends on `hoskinator-core`; the reverse never happens (ADR-0005). Built behind the `ai`
//! cargo feature on the `hoskinator` binary. Absent a configured or `ANTHROPIC_API_KEY`-set key,
//! `Config::resolve` returns `None` and the caller reports AI as unconfigured rather than erroring.

pub mod assess;
pub mod config;
pub mod suggest;
pub mod transport;

pub use assess::{AssessError, Assessment, Score, Suggestion, assess};
pub use config::Config;
pub use suggest::{DraftBullet, SuggestError, suggest_bullets};
pub use transport::{AnthropicTransport, Transport, TransportError};
