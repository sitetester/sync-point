use crate::api::response::ApiResponse;
use std::collections::HashMap;

use crate::app::App;
use log::{debug, error};
use parking_lot::RwLock;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::State;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// Type alias for our shared state.
/// Uses `parking_lot::RwLock` for better performance than `std::sync::RwLock`.
/// Outer `Arc` is not needed, because Rocket's State<T> already provides the sharing mechanism we need
/// Without inner `Arc`, we wouldn't be able to apply `.cloned()`
/// `RwLock` itself provides thread-safe sharing
pub type WaitPoints = RwLock<HashMap<String, Arc<WaitPoint>>>;

/// Represents a synchronization point where two parties can meet
pub struct WaitPoint {
    /// Notifies the first waiting party when the second party arrives
    pub notify: Notify,
    /// Atomic (thread-safe) counter to track how many parties have arrived (0, 1, or 2). Single CPU instruction, never blocks
    /// `Mutex` is overkill for simple counter, requires kernel-level locking/resources, threads block waiting for lock
    pub parties_count: AtomicUsize,
}

impl WaitPoint {
    pub(crate) fn new() -> Self {
        Self {
            notify: Notify::new(),
            parties_count: AtomicUsize::new(0),
        }
    }
}

/// Contains logic for handing the main route (/wait-for-second-party/<unique_id>)
pub struct SyncService {
    pub wait_points: WaitPoints,
}

/// This satisfies Clippy's suggestion
impl Default for SyncService {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncService {
    pub fn new() -> Self {
        Self {
            wait_points: RwLock::new(HashMap::new()),
        }
    }

    /// Handles logic when first party arrives. It will wait for a notification within timeout
    /// & return either timeout or welcome message
    ///
    /// # Arguments
    /// * `unique_id` - A string identifier for matching parties
    /// * `point: Arc<WaitPoint>` - The newly created wait point
    /// * `state` - Application state containing the sync service
    ///
    /// # Returns
    /// a `Custom<Json<ApiResponse>>` with:
    /// * HTTP Status code indicating relevant success/failure reason
    /// * JSON response with success/error/timeout status and a friendly message
    pub async fn handle_first_party(
        &self,
        unique_id: &str,
        point: Arc<WaitPoint>,
        state: &State<App>,
    ) -> Custom<Json<ApiResponse>> {
        // Wait for a notification with a timeout
        // A future which completes when `notify_one()` or `notify_waiters()` is called
        let result = tokio::time::timeout(
            Duration::from_secs(state.timeout.as_secs()),
            point.notify.notified(),
        )
        .await; // Execution suspends here
        
        if let Err(e) = self.cleanup_wait_point(unique_id) {
            return e;
        }

        match result {
            Ok(_) => {
                debug!("Notification received for unique_id: {}", unique_id);
                Custom(
                    Status::Ok,
                    Json(ApiResponse::success("Welcome! (first party)", unique_id)),
                )
            }
            Err(_) => Custom(
                Status::RequestTimeout,
                Json(ApiResponse::timeout(state.timeout, unique_id))
            )
        }
    }

    /// Handles logic when second party arrives for the same unique endpoint.
    /// It will then notify the first party and return a welcome message
    ///
    /// # Arguments
    /// * `unique_id` - A string identifier for matching parties
    /// * `point: Arc<WaitPoint>` - The existing wait point created for first party
    /// * `state` - Application state containing the timeout & sync service
    ///
    /// # Returns
    /// a `Custom<Json<ApiResponse>>` with:
    /// * HTTP Status code indicating relevant success/failure reason
    /// * JSON response with success/error/timeout status and a friendly message
    pub fn handle_second_party(
        &self,
        unique_id: &str,
        point: Arc<WaitPoint>,
    ) -> Custom<Json<ApiResponse>> {
        debug!("Second party arrived for unique_id: {}", unique_id);
        point.notify.notify_one();

        Custom(
            Status::Ok,
            Json(ApiResponse::success("Welcome! (second party)", unique_id)),
        )
    }

    /// Handles logic when more than 2 parties try to join the same unique endpoint.
    ///
    /// In general, this should never happen, since after second party has notified the first,
    /// the third party should be considered by the system as freshly joined party (first party)
    /// because the relevant parties count is reset by the first.
    ///
    /// # Arguments
    /// * `unique_id` - A string identifier for matching parties
    /// * `previous` - Party count indicator
    ///
    /// # Returns
    /// a `Custom<Json<ApiResponse>>` with:
    /// * HTTP Status code indicating relevant success/failure reason
    /// * JSON response with success/error/timeout status and a friendly message
    pub fn handle_extra_party(
        &self,
        unique_id: &str,
        previous: usize,
    ) -> Custom<Json<ApiResponse>> {
        debug!(
            "Unexpected party count {} for unique_id: {}",
            previous, unique_id
        );
        Custom(
            Status::Conflict,
            Json(ApiResponse::error("Only 2 parties allowed at a time")),
        )
    }

    /// Removes a wait point from the service state.
    ///
    /// # Arguments
    /// * `unique_id` - The unique identifier of the wait point to remove
    ///
    /// # Returns
    /// * `Ok(())` - If the wait point was successfully removed or didn't exist
    /// * `Err(Custom<Json<ApiResponse>>>)` - Relevant error info
    fn cleanup_wait_point(&self, unique_id: &str) -> Result<(), Custom<Json<ApiResponse>>> {
        match self.wait_points.try_write() {
            Some(mut points) => {
                if points.remove(unique_id).is_some() {
                    debug!("Cleaned up wait point for unique_id: {}", unique_id);
                }
                Ok(())
            }
            None => {
                error!(
                    "Failed to acquire write lock for cleanup of wait point: {}",
                    unique_id
                );
                Err(ApiResponse::service_unavailable())
            }
        }
    }

    /// Gets an existing wait point or creates a new one if it doesn't exist.
    ///
    /// # Arguments
    /// * `unique_id` - The unique identifier for the wait point
    ///
    /// # Returns
    /// * `Ok(Arc<WaitPoint>)` - The existing or newly created wait point
    /// * `Err(Custom<Json<ApiResponse>>>)` - Relevant error info
    pub fn get_or_create_point(
        &self,
        unique_id: &str,
    ) -> Result<Arc<WaitPoint>, Custom<Json<ApiResponse>>> {
        // Try to get existing point with a non-blocking read (deadlock prevention)
        if let Some(guard) = self.wait_points.try_read() {
            // `.cloned` will turn `&Arc<WaitPoint>` into `Arc<WaitPoint>`
            if let Some(point) = guard.get(&unique_id.to_owned()).cloned() {
                debug!("Wait point found for unique_id: {}", unique_id);
                return Ok(point);
            }
            // The lock is automatically released when `guard` goes out of scope
        } else {
            error!(
                "Failed to acquire read lock for cleanup of wait point: {}",
                unique_id
            );
            return Err(ApiResponse::service_unavailable());
        }

        // Create new point otherwise
        match self.wait_points.try_write() {
            Some(mut points) => {
                // If write lock acquired
                // `points  is a mutable reference to the HashMap inside the lock
                let point = Arc::new(WaitPoint::new());
                // `point.clone()` because we want to return this `point` (pointer) eventually
                // Both refer to the same WaitPoint instance (actual WaitPoint data lives on the heap)
                let point_clone = point.clone();
                // The HashMap needs to own a reference to the WaitPoint
                points.insert(unique_id.to_owned(), point_clone);
                debug!("Created new wait point for unique_id: {}", unique_id);
                Ok(point)
            }

            None => Err(ApiResponse::service_unavailable()),
        }
    }
}
