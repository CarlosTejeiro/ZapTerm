#![forbid(unsafe_code)]

//! `core-config` — application settings, themes, and profile management.
//!
//! Owns the configuration schema, theme definitions, and terminal profiles.
//! All settings are serialisable and stored in `core-persistence`; this crate
//! provides the typed Rust representations and defaults.

pub mod error;
