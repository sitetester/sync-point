use crate::api::response::ApiError;
use config::File;
use config::{Config, ConfigError, Environment, FileFormat};
use log::{debug};
use parking_lot::RwLock;
use std::collections::HashMap;
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
    /// Note: `Mutex` is overkill for simple counter, requires kernel-level locking/resources, threads block waiting for lock
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

/// Application state shared across all routes
pub struct AppState {
    pub wait_points: WaitPoints,
    pub timeout: Duration,
}

impl AppState {
    const MIN_TIMEOUT: u64 = 5;
    const MAX_TIMEOUT: u64 = 300;
    const DEFAULT_TIMEOUT: u64 = 10;

    /// Creates a new instance of the application state with configuration.
    ///
    /// # Arguments
    /// * `config_path` - Optional path to TOML configuration file
    ///
    /// Configuration can also be provided via  `APP_` prefixed environment variables
    ///
    /// # Returns
    /// * `Ok(AppState)` - Successfully initialized application state
    /// * `Err(ConfigError)` - If configuration is invalid or file cannot be read
    pub fn new(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let mut builder = Config::builder().set_default("timeout", Self::DEFAULT_TIMEOUT)?;

        // Add config file source if path is provided
        if let Some(path) = config_path {
            builder = builder.add_source(File::new(path, FileFormat::Toml).required(true));
        }

        let config = builder
            .add_source(File::new("config", FileFormat::Toml).required(false))
            // e.g. APP_TIMEOUT=30, check relevant `test_app_env_timeout` test below
            .add_source(Environment::with_prefix("APP"))
            .build()?;

        let timeout_secs: u64 = config.get("timeout")?;
        Self::is_valid_timeout(timeout_secs)?;

        let wait_points: WaitPoints = RwLock::new(HashMap::new());
        let app_state = Self {
            wait_points,
            timeout: Duration::from_secs(timeout_secs),
        };

        debug!("app_state.timeout: {:?}", app_state.timeout);
        Ok(app_state)
    }

    /// Validates that the timeout value is within acceptable bounds.
    ///
    /// # Arguments
    /// * `timeout` - The timeout value in seconds
    ///
    /// # Returns
    /// * `Ok(())` - If timeout is within [MIN_TIMEOUT, MAX_TIMEOUT] range
    /// * `Err(ConfigError)` - If timeout is outside the valid range
    fn is_valid_timeout(timeout: u64) -> Result<(), ConfigError> {
        if timeout < Self::MIN_TIMEOUT {
            return Err(ConfigError::Message(format!(
                "Timeout cannot be less than {} seconds",
                Self::MIN_TIMEOUT
            )));
        }
        if timeout > Self::MAX_TIMEOUT {
            return Err(ConfigError::Message(format!(
                "timeout cannot exceed {} seconds",
                Self::MAX_TIMEOUT
            )));
        }
        Ok(())
    }

    /// Removes a wait point from the application state.
    ///
    /// # Arguments
    /// * `unique_id` - The unique identifier of the wait point to remove
    ///
    /// # Returns
    /// * `Ok(())` - If the wait point was successfully removed or didn't exist
    /// * `Err(ApiError)` - If failed to acquire write lock
    pub fn cleanup_wait_point(&self, unique_id: &str) -> Result<(), ApiError> {
        match self.wait_points.try_write() {
            Some(mut points) => {
                if points.remove(unique_id).is_some() {
                    debug!("Cleaned up wait point for unique_id: {}", unique_id);
                }
                Ok(())
            }
            None => {
                debug!(
                    "Failed to acquire write lock for cleanup of wait point: {}",
                    unique_id
                );
                Err(ApiError::LockError(
                    "Failed to acquire write lock for cleanup".into(),
                ))
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
    /// * `Err(ApiError)` - If failed to acquire read or write lock
    pub fn get_or_create_point(&self, unique_id: &str) -> Result<Arc<WaitPoint>, ApiError> {
        // Try to get existing point
        // With an attempt to acquire a read lock without blocking (deadlock prevention)
        // Contrary `read()` will block until lock is available
        if let Some(guard) = self.wait_points.try_read() {
            // `.cloned` will turn `&Arc<WaitPoint>` into `Arc<WaitPoint>`
            if let Some(point) = guard.get(&unique_id.to_owned()).cloned() {
                return Ok(point);
            }
            // The lock is automatically released when `guard` goes out of scope
        } else {
            return Err(ApiError::LockError("Failed to acquire read lock".into()));
        }

        // Create new point otherwise
        self
            .wait_points
            .try_write() // returns None otherwise
            .map(|mut points| {
                // `points  is a mutable reference to the HashMap inside the lock
                let point = Arc::new(WaitPoint::new());
                // `point.clone()` because we want to return this `point` (pointer) eventually
                // Both refer to the same WaitPoint instance (actual WaitPoint data lives on the heap)
                let point_clone = point.clone();
                // The HashMap needs to own a reference to the WaitPoint
                points.insert(unique_id.to_owned(), point_clone);
                point // Some(WriteGuard) -> Some(Arc<WaitPoint))
            })
            .ok_or_else(|| ApiError::LockError("Failed to acquire write lock".into()))
    }
}


#[cfg(test)]
mod tests {
    use crate::api::app_state::AppState;
    use config::ConfigError;
    use serial_test::serial;
    use std::time::Duration;

    #[tokio::test]
    #[serial]
    async fn test_app_default_timeout() -> Result<(), ConfigError> {
        // Without config file
        let state = AppState::new(None)?;
        assert_eq!(state.timeout, Duration::from_secs(AppState::DEFAULT_TIMEOUT));
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_app_config_file_timeout() -> Result<(), ConfigError> {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, "timeout = 20")
            .await
            .expect("Unable to write config file");

        let state = AppState::new(Some(config_path.to_str().unwrap()))?;
        assert_eq!(state.timeout, Duration::from_secs(20));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_app_env_timeout() -> Result<(), ConfigError> {
        std::env::set_var("APP_TIMEOUT", "15");

        let state = AppState::new(None)?;
        assert_eq!(state.timeout, Duration::from_secs(15));

        std::env::remove_var("APP_TIMEOUT"); // reset
        Ok(())
    }
}
