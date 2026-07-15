//! Unified CLI error type with a documented exit-code contract
//! (docs/BACKLOG.md 3.2): each failure class maps to a distinct, stable
//! process exit code so scripts can branch on `$?` without parsing text.

use std::fmt;

use crate::catalog::CatalogError;
use crate::download::DownloadError;

#[derive(Debug)]
pub enum AppError {
    Network(String),
    RepoNotFound(String),
    NoGgufFiles(String),
    Download(String),
}

impl AppError {
    /// Process exit code for this error class. `0` is reserved for success
    /// and is never returned here.
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::Network(_) => 2,
            AppError::RepoNotFound(_) => 3,
            AppError::NoGgufFiles(_) => 4,
            AppError::Download(_) => 5,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Network(msg) => write!(f, "{msg}"),
            AppError::RepoNotFound(msg) => write!(f, "{msg}"),
            AppError::NoGgufFiles(msg) => write!(f, "{msg}"),
            AppError::Download(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<CatalogError> for AppError {
    fn from(err: CatalogError) -> Self {
        match err {
            CatalogError::RepoNotFound(_) => AppError::RepoNotFound(err.to_string()),
            CatalogError::NoGgufFiles(_) => AppError::NoGgufFiles(err.to_string()),
            CatalogError::Network(_) | CatalogError::InvalidResponse(_) => {
                AppError::Network(err.to_string())
            }
        }
    }
}

impl From<DownloadError> for AppError {
    fn from(err: DownloadError) -> Self {
        AppError::Download(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_distinct_per_class() {
        let codes = [
            AppError::Network("x".into()).exit_code(),
            AppError::RepoNotFound("x".into()).exit_code(),
            AppError::NoGgufFiles("x".into()).exit_code(),
            AppError::Download("x".into()).exit_code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len(), "exit codes must be distinct");
        assert!(codes.iter().all(|&c| c != 0), "no error may exit 0");
    }

    #[test]
    fn catalog_repo_not_found_maps_to_app_error() {
        let err: AppError = CatalogError::RepoNotFound("acme/x".into()).into();
        assert!(matches!(err, AppError::RepoNotFound(_)));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn catalog_no_gguf_files_maps_to_app_error() {
        let err: AppError = CatalogError::NoGgufFiles("acme/x".into()).into();
        assert!(matches!(err, AppError::NoGgufFiles(_)));
        assert_eq!(err.exit_code(), 4);
    }
}
