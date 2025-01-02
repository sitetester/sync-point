// It automatically acts as the entry point (root of module tree) for declaring and organizing modules.
// This eliminates the need to manually declare `mod api;` in `main.rs`.
// Instead, `lib.rs` defines all of project's modules, which can be accessed
// from anywhere including `main.rs` or tests
use crate::api::routes::{index, wait_for_party};
use app::App;
use log::debug;
use rocket::{self, routes, Build, Rocket};

// Public modules available to other crates
pub mod api;
pub mod app;

/// Builds and configures a Rocket application instance.  
/// Accessible from application as well as tests
pub fn build_rocket() -> Rocket<Build> {
    let path = "config.toml";
    let app = if std::path::Path::new(path).exists() {
        debug!("{} found", path);
        App::new(Some(path)).expect("Failed to initialize App with config")
    } else {
        debug!("{} not found", path);
        App::new(None).expect("Failed to initialize App with defaults")
    };

    rocket::build()
        // Attach our application state to Rocket's managed state
        // This makes the App available to all route handlers
        .manage(app)
        // Mounts a collection of routes at the base path "/"
        .mount("/", routes![index, wait_for_party])
}
