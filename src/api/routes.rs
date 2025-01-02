use crate::api::response::ApiResponse;
use crate::app::App;
use log::debug;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use std::sync::atomic::Ordering;

/// Handles GET requests to the root endpoint "/"
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
///
/// # Arguments
/// * `unique_id` - A string identifier for matching parties
/// * `state` - Rocket managed App instance containing synchronization data
///
/// # Returns
/// a `Custom<Json<ApiResponse>>` with:
/// * HTTP status code indicating relevant success/failure reason
/// * JSON response with success/error/timeout status and a friendly message
#[post("/wait-for-second-party/<unique_id>")]
pub async fn wait_for_party(unique_id: &str, state: &State<App>) -> Custom<Json<ApiResponse>> {
    debug!("Wait request received for unique_id: {}", unique_id);

    let point = match state.sync_service.get_or_create_point(unique_id) {
        Ok(point) => point,
        Err(response) => return response,
    };

    let previous = point.parties_count.fetch_add(1, Ordering::SeqCst);
    match previous {
        0 => {
            state
                .sync_service
                .handle_first_party(unique_id, point, state)
                .await
        }
        1 => state.sync_service.handle_second_party(unique_id, point),
        _ => state.sync_service.handle_extra_party(unique_id, previous),
    }
}
