// It automatically acts as the entry point for declaring and organizing modules.
// This eliminates the need to manually declare `mod api;` in `main.rs`.
// Instead, `lib.rs` defines all of  project's modules, which can be accessed
// by any binary (`main.rs`) or test module in the project.

use crate::api::app_state::AppState;
use crate::api::routes::{index, wait_for_party};
use log::debug;
use rocket::{self, routes, Build, Rocket};

pub mod api; // Declare the api module

/// Builds and configures a Rocket application instance.  
/// Accessible from application as well as tests
pub fn build_rocket() -> Rocket<Build> {
    let path = "config.toml";
    let state_result = if std::path::Path::new(path).exists() {
        debug!("{} found", path);
        AppState::new(Some(path))
    } else {
        debug!("{} not found", path);
        AppState::new(None)
    };
    let app_state = state_result.expect("Failed to initialize AppState");

    rocket::build()
        // Attach our application state to Rocket's managed state
        // This makes the AppState available to all route handlers
        .manage(app_state)
        // Mounts a collection of routes at the base path "/"
        .mount("/", routes![index, wait_for_party])
}
