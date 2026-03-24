mod api;
mod config;
mod core_layer;
mod imessage;
mod storage;
mod tray;

use std::sync::Arc;

use api::handlers::AppState;
use core_layer::backend::MessageBackend;
use core_layer::webhook::WebhookDispatcher;
use imessage::backend::IMessageBackend;
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("aimessage=info".parse().unwrap()),
        )
        .init();

    let config = match config::Config::load() {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    let api_key = config.auth.api_key.clone();

    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        "Config loaded"
    );

    // Tray icon on main thread, server on background thread
    tray::run(api_key, config);
}

pub async fn run_server(config: config::Config) {
    // Init storage
    let db_path = config::Config::config_dir().join("aimessage.db");
    let storage = Arc::new(
        storage::sqlite::Storage::new(&db_path).expect("Failed to initialize database"),
    );
    tracing::info!(path = %db_path.display(), "Database initialized");

    // Check Automation permission
    if let Err(e) = imessage::applescript::check_automation_permission().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    tracing::info!("Automation permission verified");

    // Init iMessage backend
    let backend = Arc::new(IMessageBackend::new(
        config.imessage.clone(),
        storage.clone(),
    ));

    // Start backend — begins polling chat.db
    let event_sender = backend
        .start()
        .await
        .expect("Failed to start iMessage backend");

    // Start webhook dispatcher (subscribes to the broadcast channel)
    let dispatcher = WebhookDispatcher::new(storage.clone());
    dispatcher.spawn(event_sender.subscribe());

    // Build app state and router
    let state = Arc::new(AppState {
        backend: backend as Arc<dyn MessageBackend>,
        storage: storage.clone(),
        event_sender,
        api_key: config.auth.api_key.clone(),
    });

    let app = api::routes::build_router(state, config.auth.api_key);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!(addr = %addr, "Server starting");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    tracing::info!("Shutdown signal received, draining connections...");
}
