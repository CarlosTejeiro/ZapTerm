pub mod activity;
pub mod backup;
pub mod bulk;
pub mod cmdlists;
pub mod data_dir;
pub mod folders;
pub mod forwards;
pub mod groups;
pub mod highlight;
pub mod hints;
pub mod logging;
pub mod login_scripts;
pub mod master;
pub mod scripts;
pub mod sessions;
pub mod settings;
pub mod sftp;
pub mod shell;
pub mod snippets;
pub mod sync;
pub mod transfer;
pub mod triggers;
// Compiled on every Unix so its tests run in CI (which has no macOS runner);
// only the macOS build calls it.
#[cfg(unix)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod mac_update;
pub mod update;
pub mod vault;
