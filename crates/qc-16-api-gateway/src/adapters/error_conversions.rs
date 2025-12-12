//! Error conversions from infrastructure types.
//!
//! These conversions involve I/O types and belong in the adapters layer.

use crate::domain::ApiError;

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::internal(e.to_string())
    }
}
