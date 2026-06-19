use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("missing or invalid adapter api key")]
    Unauthorized,
    #[error("request stream mode is not supported in the MVP")]
    StreamUnsupported,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("upstream request failed: {0}")]
    Upstream(String),
}

impl IntoResponse for AdapterError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::StreamUnsupported => StatusCode::NOT_IMPLEMENTED,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
        };
        let body = Json(json!({
            "error": {
                "message": self.to_string(),
                "type": "adapter_error"
            }
        }));
        (status, body).into_response()
    }
}
