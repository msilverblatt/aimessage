mod config;
mod core_layer;

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
}
