#![forbid(unsafe_code)]

//! `core-terminal` — VT sequence parsing for the Rust side of the pipeline.
//!
//! Uses `vte` (Alacritty's VT parser) to process byte streams from PTY/SSH/Telnet.
//! The parsed output drives the keyword-highlighting engine (Bloque 5).
//!
//! Note: visual rendering is handled by xterm.js in the frontend; this crate
//! maintains a parallel parse state for Rust-side consumers only.

pub mod error;
pub mod parser;

pub use parser::VteParser;
