//! Error type shared by all checkpoint-v1 operations.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("corrupt checkpoint at {path}: expected blake3 {expected}, got {actual}")]
    Corrupt {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("bad header magic in {path}")]
    BadMagic { path: PathBuf },
    #[error("missing manifest field `{0}`")]
    MissingField(String),
    #[error("grid dimensions exceed u16 limit in {path}")]
    GridTooLarge { path: PathBuf },
    #[error("cbor: {0}")]
    Cbor(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<std::io::Error> for CheckpointError {
    fn from(source: std::io::Error) -> Self {
        CheckpointError::Io {
            path: PathBuf::new(),
            source,
        }
    }
}
