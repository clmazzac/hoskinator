//! Hoskinator's core engine.
//!
//! Owns the Master Store, git mediation via libgit2 (ADR-0004), rendercv YAML editing, and
//! the core JSON-RPC methods.
//!
//! Contains no LLM code and reads no LLM API key — this crate is fully functional standalone
//! (ADR-0005).

pub mod application;
pub mod bullet;
pub mod config;
pub mod entry;
pub mod google_auth;
pub mod home;
pub mod job_description;
pub mod lineage;
pub mod profile;
pub mod render;
pub mod repository;
pub mod resume;
pub mod search;
pub mod section;
pub mod sheets;
pub mod store;
pub mod tailoring;
pub mod workspace;
