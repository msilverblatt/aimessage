use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Backend unavailable: {0}")]
    Unavailable(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Feature unavailable: {0}")]
    FeatureUnavailable(String),
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Backend(BackendError::Unavailable(_)) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            ApiError::Backend(BackendError::NotFound(_)) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            ApiError::Backend(BackendError::InvalidRequest(_)) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            ApiError::Backend(BackendError::FeatureUnavailable(_)) => {
                (StatusCode::NOT_IMPLEMENTED, self.to_string())
            }
            ApiError::Backend(BackendError::RequestFailed(_)) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            ApiError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };

        let body = serde_json::to_string(&json!({ "error": message })).unwrap();
        (status, [("content-type", "application/json")], body).into_response()
    }
}
