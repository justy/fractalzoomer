/// WebSocket handler for rendering strip requests

use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use crate::colour::generate_palette;
use crate::mandelbrot::render_strip;

/// Request message from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    Render(RenderRequest),
    Benchmark(BenchmarkRequest),
}

#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub frame_id: u64,
    pub width: u32,
    pub y_start: u32,
    pub y_end: u32,
    pub total_height: u32,
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub max_iterations: u32,
}

#[derive(Debug, Deserialize)]
pub struct BenchmarkRequest {
    pub width: u32,
    pub height: u32,
}

/// Response message to client
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ServerMessage {
    Strip(StripResponse),
    BenchmarkResult(BenchmarkResponse),
    Error(ErrorResponse),
}

#[derive(Debug, Serialize)]
pub struct StripResponse {
    pub frame_id: u64,
    pub y_start: u32,
    pub y_end: u32,
    pub compute_ms: u64,
    pub data: String, // Base64 encoded RGB
}

#[derive(Debug, Serialize)]
pub struct BenchmarkResponse {
    pub compute_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub message: String,
}

/// Shared state for workers
pub struct WorkerState {
    pub palette: Vec<(u8, u8, u8)>,
}

impl WorkerState {
    pub fn new() -> Self {
        Self {
            palette: generate_palette(2048),
        }
    }
}

impl Default for WorkerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a WebSocket connection
pub async fn handle_socket(socket: WebSocket, state: Arc<WorkerState>) {
    let (mut sender, mut receiver) = socket.split();

    tracing::info!("New WebSocket connection established");

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("WebSocket receive error: {}", e);
                break;
            }
        };

        // Handle text messages only
        let text = match msg {
            Message::Text(text) => text,
            Message::Close(_) => {
                tracing::info!("WebSocket closed by client");
                break;
            }
            Message::Ping(data) => {
                if let Err(e) = sender.send(Message::Pong(data)).await {
                    tracing::error!("Failed to send pong: {}", e);
                    break;
                }
                continue;
            }
            _ => continue,
        };

        // Parse the message
        let client_msg: ClientMessage = match serde_json::from_str(&text) {
            Ok(msg) => msg,
            Err(e) => {
                let error = ServerMessage::Error(ErrorResponse {
                    message: format!("Invalid message format: {}", e),
                });
                let _ = sender
                    .send(Message::Text(serde_json::to_string(&error).unwrap().into()))
                    .await;
                continue;
            }
        };

        // Process the request
        let response = match client_msg {
            ClientMessage::Render(req) => process_render_request(req, &state),
            ClientMessage::Benchmark(req) => process_benchmark_request(req, &state),
        };

        // Send response
        let response_text = serde_json::to_string(&response).unwrap();
        if let Err(e) = sender.send(Message::Text(response_text.into())).await {
            tracing::error!("Failed to send response: {}", e);
            break;
        }
    }

    tracing::info!("WebSocket connection closed");
}

fn process_render_request(req: RenderRequest, state: &WorkerState) -> ServerMessage {
    let start = Instant::now();

    // Validate request
    if req.y_end <= req.y_start || req.width == 0 || req.total_height == 0 {
        return ServerMessage::Error(ErrorResponse {
            message: "Invalid strip dimensions".to_string(),
        });
    }

    // Render the strip
    let pixels = render_strip(
        req.width,
        req.y_start,
        req.y_end,
        req.total_height,
        req.center_x,
        req.center_y,
        req.zoom,
        req.max_iterations,
        &state.palette,
    );

    let compute_ms = start.elapsed().as_millis() as u64;

    // Encode as base64
    let data = base64::engine::general_purpose::STANDARD.encode(&pixels);

    ServerMessage::Strip(StripResponse {
        frame_id: req.frame_id,
        y_start: req.y_start,
        y_end: req.y_end,
        compute_ms,
        data,
    })
}

fn process_benchmark_request(req: BenchmarkRequest, state: &WorkerState) -> ServerMessage {
    let start = Instant::now();

    // Render a small test region
    let _ = render_strip(
        req.width,
        0,
        req.height,
        req.height,
        -0.5, // Standard Mandelbrot view
        0.0,
        1.0,
        256, // Fixed iterations for benchmark
        &state.palette,
    );

    let compute_ms = start.elapsed().as_millis() as u64;

    ServerMessage::BenchmarkResult(BenchmarkResponse { compute_ms })
}
