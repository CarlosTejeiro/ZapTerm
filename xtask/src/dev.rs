use std::path::PathBuf;
use std::process::Command;

/// `cargo xtask dev` — start the Vite dev server and the Tauri app together.
///
/// Delegates to `cargo tauri dev`, run from `crates/app/`. That reads
/// `crates/app/tauri.conf.json`, whose `beforeDevCommand` boots the Vite dev
/// server in `frontend/` (on the `devUrl` port) and whose `devUrl` points the
/// webview at it. Tauri waits for the server, builds the Rust binary, and
/// launches — one command, no manual two-terminal orchestration.
pub fn run(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Workspace root = parent of the xtask crate dir, so this works no matter
    // which directory `cargo xtask` was invoked from.
    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("cannot locate workspace root")?
        .join("crates/app");

    let status = Command::new("cargo")
        .args(["tauri", "dev"])
        .current_dir(&app_dir)
        .status()
        .map_err(|e| {
            format!(
                "failed to spawn `cargo tauri dev` (install the CLI with \
                 `cargo install tauri-cli --version '^2'`): {e}"
            )
        })?;

    if !status.success() {
        return Err("cargo tauri dev exited with a non-zero status".into());
    }
    Ok(())
}
