#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SQLite error: {0}")]
    Rusqlite(#[from] rusqlite::Error),

    #[error("record not found")]
    NotFound,

    #[error("this database was written by a newer ZapTerm (schema {found}, this version reads up to {supported}) — update ZapTerm to open it")]
    NewerSchema { found: i64, supported: i64 },

    #[error("{0}")]
    InvalidPath(String),
}
