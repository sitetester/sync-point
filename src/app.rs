use crate::api::sync_service::SyncService;
use config::File;
use config::{Config, ConfigError, Environment, FileFormat};
use log::debug;
use std::time::Duration;

/// Application state container managing timeout and sync Service
/// Rocket manages the sharing between routes via State<App>
/// Each route receives a thread-safe reference (`&State<App>`) to this instance
pub struct App {
    /// Used for a notification from 2nd party with this timeout value
    pub timeout: Duration,
    /// A service holding parties sync logic
    pub sync_service: SyncService,
}

impl App {
    // Currently hardcoded values, but could be configurable from outside.
    const MIN_TIMEOUT: u64 = 5;
    const MAX_TIMEOUT: u64 = 300;
    const DEFAULT_TIMEOUT: u64 = 10;

    /// Creates a new instance of the application with configuration.
    ///
    /// Configuration can be provided via
    /// - TOML config file (optional)
    /// - `APP_` prefix environment variable
    ///
    /// # Arguments
    /// * `config_path` - Optional path to TOML config file. See tests how we could pass a custom path.
    ///
    /// # Returns
    /// * `Ok(App)` - Successfully initialized application
    /// * `Err(ConfigError)` - If configuration is invalid or file cannot be read
    pub fn new(config_path: Option<&str>) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .set_default("timeout", Self::DEFAULT_TIMEOUT)?
            .add_source(match config_path {
                Some(path) => File::new(path, FileFormat::Toml).required(true),
                None => File::new("config", FileFormat::Toml).required(false),
            })
            // e.g. APP_TIMEOUT=30, check relevant `test_app_env_timeout` test below
            .add_source(Environment::with_prefix("APP"))
            .build()?;

        let timeout_secs: u64 = config.get("timeout")?;
        Self::validate_timeout(timeout_secs)?;

        let app = Self {
            timeout: Duration::from_secs(timeout_secs),
            sync_service: SyncService::new(),
        };

        debug!("app.timeout: {:?}", app.timeout);
        Ok(app)
    }

    /// Validates that the timeout value is within acceptable bounds.
    ///
    /// # Arguments
    /// * `timeout` - The timeout value in seconds
    ///
    /// # Returns
    /// * `Ok(())` - If timeout is within [MIN_TIMEOUT, MAX_TIMEOUT] range
    /// * `Err(ConfigError)` - If timeout is outside the valid range
    fn validate_timeout(timeout: u64) -> Result<(), ConfigError> {
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
}

/// The `#[serial]` attribute is used to mark tests that should run sequentially
/// This is to avoid timeout conflicts occurring from config file or env vars
#[cfg(test)]
mod tests {
    use crate::app::App;
    use config::ConfigError;
    use serial_test::serial;
    use std::time::Duration;

    #[tokio::test]
    #[serial]
    async fn test_app_default_timeout() -> Result<(), ConfigError> {
        // Without config file
        let app = App::new(None)?;
        assert_eq!(app.timeout, Duration::from_secs(App::DEFAULT_TIMEOUT));
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

        let app = App::new(Some(config_path.to_str().unwrap()))?;
        assert_eq!(app.timeout, Duration::from_secs(20));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_app_env_timeout() -> Result<(), ConfigError> {
        std::env::set_var("APP_TIMEOUT", "15");

        let app = App::new(None)?;
        assert_eq!(app.timeout, Duration::from_secs(15));

        std::env::remove_var("APP_TIMEOUT"); // reset
        Ok(())
    }
}
