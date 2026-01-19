mod colour;
mod mandelbrot;
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

use worker::WorkerState;

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

    // Create shared worker state
    let state = Arc::new(WorkerState::new());

    // CORS configuration for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health_handler))
        .nest_service("/", ServeDir::new("static").append_index_html_on_directories(true))
        .layer(cors)
        .with_state(state);

    // Get port from environment or default to 8080
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Fractal Zoomer worker listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// WebSocket upgrade handler
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WorkerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket: WebSocket| worker::handle_socket(socket, state))
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}
