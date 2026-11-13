//! Importers for third-party session formats. Each importer parses its
//! source into the native [`crate::commands::transfer::ExportFile`] shape,
//! so the import pipeline (`apply_import`) handles id remapping, duplicate
//! skipping and the summary identically for every source.

pub mod common;
pub mod mobaxterm;
pub mod putty;
pub mod securecrt;
pub mod ssh_config;
