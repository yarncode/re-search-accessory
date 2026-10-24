use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("không gọi được thegioiic: {0}")]
    Transport(String),
    #[error("{message}")]
    Upstream { status: u16, message: String },
    #[error("ảnh không tồn tại")]
    NotFound,
    #[error("{0}")]
    Store(String),
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Transport(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Transport(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::Upstream { status, message } => (
                StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
                message,
            ),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Store(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
