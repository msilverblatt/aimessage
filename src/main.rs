mod api;
mod backends;
mod config;
mod core_layer;
mod storage;

use std::sync::Arc;

use api::handlers::AppState;
use backends::bluebubbles::BlueBubblesBackend;
use core_layer::backend::MessageBackend;
use core_layer::webhook::WebhookDispatcher;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
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

    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        backend = %config.backend.backend_type,
        "Config loaded"
    );

    // Init storage
    let db_path = config::Config::config_dir().join("aimessage.db");
    let storage = Arc::new(
        storage::sqlite::Storage::new(&db_path).expect("Failed to initialize database"),
    );
    tracing::info!(path = %db_path.display(), "Database initialized");

    // Init backend
    let bb_config = config
        .backend
        .bluebubbles
        .clone()
        .expect("BlueBubbles config missing");
    let backend = Arc::new(BlueBubblesBackend::new(bb_config, config.server.port));

    // Start backend and get message receiver
    let receiver = backend
        .start()
        .await
        .expect("Failed to start backend");

    // Start webhook dispatcher
    let dispatcher = WebhookDispatcher::new(storage.clone());
    dispatcher.spawn(receiver);

    // Build app state and router
    let state = Arc::new(AppState {
        backend: backend as Arc<dyn MessageBackend>,
        storage: storage.clone(),
    });

    let app = api::routes::build_router(state, config.auth.api_key);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!(addr = %addr, "Server starting");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .expect("Server error");
}
