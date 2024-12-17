use sync_point::api::app_state::build_rocket;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    // To set configure logging level via environment 
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug"), // Set default log level to debug
    )
    .init();

    log::info!("🚀 Starting server...");
    let rocket = build_rocket();
    rocket.launch().await?;
    Ok(())
}
