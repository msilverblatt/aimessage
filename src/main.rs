mod config;
mod core_layer;
mod storage;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env().add_directive("aimessage=info".parse().unwrap()))
        .init();

    let config = match config::Config::load() {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        backend = %config.backend.backend_type,
        "Config loaded"
    );

    let db_path = config::Config::config_dir().join("aimessage.db");
    let storage = std::sync::Arc::new(
        storage::sqlite::Storage::new(&db_path)
            .expect("Failed to initialize database")
    );
    tracing::info!(path = %db_path.display(), "Database initialized");
}
