use crate::api::app_state::{AppState, WaitPoint};
use crate::api::response::{ApiError, ApiResponse};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

#[get("/")]
pub fn index() -> &'static str {
    "Welcome to Sync Point API"
}

/// Main endpoint handler for party synchronization
///
/// When a party arrives:
/// - If they're first, they'll wait for the second party
/// - If they're second, they'll notify the first party
/// - If more parties try to join, they'll be rejected
#[post("/wait-for-second-party/<unique_id>")]
pub async fn wait_for_party(
    unique_id: &str,
    state: &State<AppState>
) -> Custom<Json<ApiResponse>> {
    let get_or_create_point = || -> Result<Arc<WaitPoint>, ApiError> {
        // Try to get existing point
        if let Some(guard) = state.wait_points.try_read() {
            if let Some(point) = guard.get(&unique_id.to_owned()).cloned() {
                return Ok(point);
            }
        } else {
            return Err(ApiError::LockError("Failed to acquire read lock".into()));
        }

        // Create new point otherwise
        state.wait_points
            .try_write()
            .map(|mut points| {
                let point = Arc::new(WaitPoint::new());
                points.insert(unique_id.to_owned(), point.clone());
                point
            })
            .ok_or_else(|| ApiError::LockError("Failed to acquire write lock".into()))
    };

    let point = match get_or_create_point() {
        Ok(point) => point,
        Err(e) => return e.into()
    };

    let previous = point.parties_count.fetch_add(1, Ordering::SeqCst);
    match previous {
        0 => handle_first_party(point, unique_id, state).await.unwrap_or_else(Into::into),
        1 => handle_second_party(point, unique_id).unwrap_or_else(Into::into),
        _ => handle_extra_party(previous, unique_id).unwrap_or_else(Into::into)
    }
}

async fn handle_first_party(
    point: Arc<WaitPoint>,
    unique_id: &str,
    state: &State<AppState>
) -> Result<Custom<Json<ApiResponse>>, ApiError> {
    // Wait for a notification with a timeout
    // A future which completes when `notify_one()` or `notify_waiters()` is called
    match tokio::time::timeout(
        Duration::from_secs(state.timeout.as_secs()),
        point.notify.notified(),
    ).await {
        Ok(_) => {
            log::debug!("Notification received for unique_id: {}", unique_id);
            state.cleanup_wait_point(unique_id)?;
            Ok(Custom(
                Status::Ok,
                Json(ApiResponse::success("Welcome! (first party)")),
            ))
        }
        Err(_) => {
            log::debug!("Timeout occurred for unique_id: {}", unique_id);
            state.cleanup_wait_point(unique_id)?;
            Err(ApiError::TimeoutError(state.timeout))
        }
    }
}

fn handle_second_party(point: Arc<WaitPoint>, unique_id: &str) -> Result<Custom<Json<ApiResponse>>, ApiError> {
    log::debug!("Second party arrived for unique_id: {}", unique_id);
    point.notify.notify_one();
    Ok(Custom(
        Status::Ok,
        Json(ApiResponse::success("Welcome! (second party)")),
    ))
}

fn handle_extra_party(previous: usize, unique_id: &str) -> Result<Custom<Json<ApiResponse>>, ApiError> {
    log::debug!(
        "Unexpected party count {} for unique_id: {}",
        previous,
        unique_id
    );
    Ok(Custom(
        Status::Conflict,
        Json(ApiResponse::error("Only 2 parties allowed at a time")),
    ))
}

