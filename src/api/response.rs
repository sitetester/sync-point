use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    /// Generates successful API response with a message and unique identifier
    ///
    /// # Arguments
    /// * `message` - A friendly welcome message
    /// * `unique_id` - A unique identifier to track the response. This helps to make it distinguish
    ///                 about which route such response was generated (otherwise it will be same generic
    ///                 welcome message for each one)
    ///
    /// # Returns
    /// `ApiResponse` instance with:
    /// * `status` set to `ResponseStatus::Success`
    /// * `message` formatted as "[unique_id] message"
    /// * `timeout_duration_sec` set to `None`. Not visible in JSON response.
    pub fn success(message: &str, unique_id: &str) -> Self {
        Self {
            status: ResponseStatus::Success,
            message: format!("[{}] {}", unique_id, message),
            timeout_duration_sec: None,
        }
    }

    /// Same as `success` response, but with additional `timeout_duration_sec` field`
    pub fn timeout(duration: Duration, unique_id: &str) -> Self {
        Self {
            status: ResponseStatus::Timeout,
            message: format!("[{}] Request timed out", unique_id),
            timeout_duration_sec: Some(duration.as_secs()),
        }
    }

    /// Will return critical error messages
    pub fn error(message: &str) -> Self {
        Self {
            status: ResponseStatus::Error,
            message: message.to_string(),
            timeout_duration_sec: None,
        }
    }

    /// A helper method to avoid repetition
    pub fn service_unavailable() -> Custom<Json<Self>> {
        Custom(
            Status::ServiceUnavailable,
            Json(Self::error("Service temporarily unavailable")),
        )
    }
}
