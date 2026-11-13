/// Application-level errors that cross the Tauri bridge to the frontend.
///
/// Must be serialisable — the frontend receives these as JSON.
#[derive(Debug, thiserror::Error, serde::Serialize)]
#[non_exhaustive]
pub enum AppError {
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
