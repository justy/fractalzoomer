/// Worker module - connects to coordinator, renders strips

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::colour::Palette;
use crate::mandelbrot::render_strip;
use crate::messages::*;

/// Heartbeat interval
const HEARTBEAT_INTERVAL_SECS: u64 = 10;

/// Reconnection delay
const RECONNECT_DELAY_SECS: u64 = 5;

/// Worker state
pub struct Worker {
    pub worker_id: String,
    pub coordinator_url: String,
    pub palette: Vec<(u8, u8, u8)>,
}

impl Worker {
    pub fn new(coordinator_url: String) -> Self {
        Self {
            worker_id: uuid::Uuid::new_v4().to_string(),
            coordinator_url,
            palette: Palette::default().generate(2048),
        }
    }

    /// Run the worker - connects to coordinator and processes work
    pub async fn run(self: Arc<Self>) {
        loop {
            tracing::info!("Connecting to coordinator at {}", self.coordinator_url);

            match self.connect_and_work().await {
                Ok(_) => {
                    tracing::info!("Disconnected from coordinator");
                }
                Err(e) => {
                    tracing::error!("Connection error: {}", e);
                }
            }

            tracing::info!("Reconnecting in {} seconds...", RECONNECT_DELAY_SECS);
            tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
        }
    }

    async fn connect_and_work(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = connect_async(&self.coordinator_url).await?;
        let (mut sender, mut receiver) = ws_stream.split();

        tracing::info!("Connected to coordinator");

        // Register with coordinator
        let register_msg = WorkerToCoordinator::Register {
            worker_id: self.worker_id.clone(),
        };
        sender.send(Message::Text(serde_json::to_string(&register_msg)?)).await?;

        // Spawn heartbeat task
        let worker_id = self.worker_id.clone();
        let heartbeat_sender = sender.reunite(receiver).expect("reunite failed");
        let (mut sender, mut receiver) = heartbeat_sender.split();

        let (heartbeat_tx, mut heartbeat_rx) = tokio::sync::mpsc::channel::<()>(1);

        let heartbeat_worker_id = worker_id.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Heartbeat will be sent via main loop
                    }
                    _ = heartbeat_rx.recv() => {
                        break;
                    }
                }
            }
        });

        // Spawn sender task
        let (send_tx, mut send_rx) = tokio::sync::mpsc::channel::<WorkerToCoordinator>(32);

        tokio::spawn(async move {
            while let Some(msg) = send_rx.recv().await {
                let text = serde_json::to_string(&msg).unwrap();
                if sender.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        });

        // Heartbeat loop
        let heartbeat_tx_clone = send_tx.clone();
        let heartbeat_worker_id = self.worker_id.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
            loop {
                interval.tick().await;
                let msg = WorkerToCoordinator::Heartbeat {
                    worker_id: heartbeat_worker_id.clone(),
                };
                if heartbeat_tx_clone.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Process messages from coordinator
        while let Some(msg) = receiver.next().await {
            let msg = match msg {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) => {
                    tracing::info!("Coordinator closed connection");
                    break;
                }
                Ok(Message::Ping(data)) => {
                    // Pong is handled automatically by tungstenite
                    continue;
                }
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
                _ => continue,
            };

            let parsed: CoordinatorToWorker = match serde_json::from_str(&msg) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Invalid coordinator message: {}", e);
                    continue;
                }
            };

            match parsed {
                CoordinatorToWorker::Registered { worker_id } => {
                    tracing::info!("Successfully registered as {}", worker_id);
                }

                CoordinatorToWorker::RunProfile { width, height } => {
                    tracing::debug!("Running profile {}x{}", width, height);
                    let compute_ms = self.run_profile(width, height);
                    let response = WorkerToCoordinator::ProfileResult {
                        worker_id: self.worker_id.clone(),
                        compute_ms,
                    };
                    let _ = send_tx.send(response).await;
                }

                CoordinatorToWorker::RenderStrip(req) => {
                    tracing::debug!("Rendering strip {} y={}..{}", req.frame_id, req.y_start, req.y_end);
                    let result = self.render_strip_request(&req);
                    let _ = send_tx.send(WorkerToCoordinator::StripResult(result)).await;
                }
            }
        }

        // Signal heartbeat to stop
        drop(heartbeat_tx);

        Ok(())
    }

    /// Run a profiling computation
    fn run_profile(&self, width: u32, height: u32) -> u64 {
        let start = Instant::now();

        // Fixed profile area - standard Mandelbrot view
        let _ = render_strip(
            width,
            0,
            height,
            height,
            -0.5,
            0.0,
            1.0,
            256,
            &self.palette,
            false,
        );

        start.elapsed().as_millis() as u64
    }

    /// Render a strip request
    fn render_strip_request(&self, req: &RenderStripRequest) -> StripResult {
        let start = Instant::now();

        // Generate palette based on request
        let palette = req.palette.generate(2048);

        let pixels = render_strip(
            req.width,
            req.y_start,
            req.y_end,
            req.total_height,
            req.center_x,
            req.center_y,
            req.zoom,
            req.max_iterations,
            &palette,
            req.colour_interior,
        );

        let compute_ms = start.elapsed().as_millis() as u64;
        let data = base64::engine::general_purpose::STANDARD.encode(&pixels);

        StripResult {
            worker_id: self.worker_id.clone(),
            frame_id: req.frame_id,
            y_start: req.y_start,
            y_end: req.y_end,
            compute_ms,
            data,
        }
    }
}
