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

    /// Creates a new instance of the application configuration
    pub fn new(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let mut builder = Config::builder().set_default("timeout", Self::DEFAULT_TIMEOUT)?;

        // Add config file source if path is provided
        if let Some(path) = config_path {
            builder = builder.add_source(File::new(path, FileFormat::Toml).required(true));
        }

        let config = builder
            .add_source(File::new("config", FileFormat::Toml).required(false))
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
