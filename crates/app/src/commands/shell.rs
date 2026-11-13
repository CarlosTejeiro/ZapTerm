//! Tiny helper to open a URL in the user's default browser, using the same
//! OS opener pattern as the "reveal folder" commands (no extra plugin).

use crate::error::AppError;

/// Open `url` in the default browser. Only http(s) URLs are allowed, so this
/// can't be coerced into launching arbitrary local programs.
#[tauri::command]
pub async fn open_external(url: String) -> Result<(), AppError> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::Internal("only http(s) URLs are allowed".into()));
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(&url)
        .spawn()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}
