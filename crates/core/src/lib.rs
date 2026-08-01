//! Hoskinator's core engine.
//!
//! Owns the Master Store, git mediation via libgit2 (ADR-0004), rendercv YAML editing, and
//! the core JSON-RPC methods.
//!
//! Contains no LLM code and reads no API key — this crate is fully functional standalone
//! (ADR-0005).

pub mod config;
pub mod home;
pub mod store;
