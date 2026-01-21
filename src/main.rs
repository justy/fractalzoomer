mod colour;
mod coordinator;
mod mandelbrot;
mod messages;
mod worker;

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use coordinator::Coordinator;
use worker::Worker;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Coordinator,
    Worker,
    Standalone,
}

#[tokio::main]
async fn main() {
    // Initialise tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fractal_zoomer=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Determine mode from environment
    let mode = match std::env::var("MODE").as_deref() {
        Ok("coordinator") => Mode::Coordinator,
        Ok("worker") => Mode::Worker,
        Ok("standalone") => Mode::Standalone,
        _ => {
            // Default based on whether COORDINATOR_URL is set
            if std::env::var("COORDINATOR_URL").is_ok() {
                Mode::Worker
            } else {
                // Default to standalone for ease of use
                Mode::Standalone
            }
        }
    };

    match mode {
        Mode::Coordinator => run_coordinator().await,
        Mode::Worker => run_worker().await,
        Mode::Standalone => run_standalone().await,
    }
}

async fn run_coordinator() {
    tracing::info!("Starting in COORDINATOR mode");

    let coordinator = Coordinator::new();

    // Start the profiling loop
    coordinator.start_profile_loop();

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/ws/worker", get(worker_ws_handler))
        .route("/ws/client", get(client_ws_handler))
        .route("/health", get(health_handler))
        .nest_service("/", ServeDir::new("static").append_index_html_on_directories(true))
        .layer(cors)
        .with_state(coordinator);

    // Get port from environment or default to 8080
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Coordinator listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn run_worker() {
    let coordinator_url = std::env::var("COORDINATOR_URL")
        .expect("COORDINATOR_URL environment variable required in worker mode");

    tracing::info!("Starting in WORKER mode, coordinator: {}", coordinator_url);

    let worker = Arc::new(Worker::new(coordinator_url));
    worker.run().await;
}

async fn run_standalone() {
    tracing::info!("Starting in STANDALONE mode (coordinator + local worker)");

    let coordinator = Coordinator::new();

    // Start the profiling loop
    coordinator.start_profile_loop();

    // Get port from environment or default to 8080
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    // Spawn a local worker that connects to this coordinator
    let worker_url = format!("ws://127.0.0.1:{}/ws/worker", port);
    let worker = Arc::new(Worker::new(worker_url));
    tokio::spawn(async move {
        // Small delay to let the server start
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        worker.run().await;
    });

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/ws/worker", get(worker_ws_handler))
        .route("/ws/client", get(client_ws_handler))
        .route("/health", get(health_handler))
        .nest_service("/", ServeDir::new("static").append_index_html_on_directories(true))
        .layer(cors)
        .with_state(coordinator);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Standalone server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// WebSocket handler for worker connections (coordinator side)
async fn worker_ws_handler(
    ws: WebSocketUpgrade,
    State(coordinator): State<Arc<Coordinator>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket: WebSocket| async move {
        coordinator.handle_worker_connection(socket).await;
    })
}

/// WebSocket handler for client connections (coordinator side)
async fn client_ws_handler(
    ws: WebSocketUpgrade,
    State(coordinator): State<Arc<Coordinator>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket: WebSocket| async move {
        coordinator.handle_client_connection(socket).await;
    })
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}
