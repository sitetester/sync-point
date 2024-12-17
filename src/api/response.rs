use serde::{Deserialize, Serialize};
use std::time::Duration;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
#[derive(Debug)]
pub enum ApiError {
    LockError(String),
    TimeoutError(Duration),
    CleanupError(String),
}

impl From<ApiError> for Custom<Json<ApiResponse>> {
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::LockError(msg) => Custom(
                Status::InternalServerError,
                Json(ApiResponse::error(&msg)),
            ),
            ApiError::TimeoutError(duration) => Custom(
                Status::RequestTimeout,
                Json(ApiResponse::timeout(duration)),
            ),
            ApiError::CleanupError(msg) => {
                log::error!("Cleanup error: {}", msg);
                Custom(
                    Status::InternalServerError,
                    Json(ApiResponse::error("Internal server error during cleanup")),
                )
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    Success,
    Timeout,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
    status: ResponseStatus,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_duration_sec: Option<u64>,
}

impl ApiResponse {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            status: ResponseStatus::Success,
            message: message.into(),
            timeout_duration_sec: None,
        }
    }

    pub fn timeout(duration: Duration) -> Self {
        Self{
            status: ResponseStatus::Timeout,
            message: "Request timed out".into(),
            timeout_duration_sec: Some(duration.as_secs()),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: ResponseStatus::Error,
            message: message.into(),
            timeout_duration_sec: None,
        }
    }
}
