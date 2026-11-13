#![forbid(unsafe_code)]

//! `core-session` — session lifecycle, state machine, and protocol-agnostic session model.
//!
//! A session is a named, saved connection configuration plus its runtime state
//! (connected/disconnected/reconnecting). This crate owns the lifecycle transitions
//! and the data types that cross the Tauri command boundary.

pub mod error;
