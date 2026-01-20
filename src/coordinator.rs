/// Coordinator module - manages workers, assigns work, assembles frames

use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

use crate::messages::*;

/// Profile dimensions - fixed area for consistent benchmarking
const PROFILE_WIDTH: u32 = 512;
const PROFILE_HEIGHT: u32 = 512;

/// How often to re-profile workers (in seconds)
const PROFILE_INTERVAL_SECS: u64 = 60;

/// Worker timeout - remove if no heartbeat in this time
const WORKER_TIMEOUT_SECS: u64 = 30;

/// Information about a connected worker
struct WorkerInfo {
    sender: mpsc::Sender<CoordinatorToWorker>,
    capability: f64,  // Higher = faster (inverse of profile time)
    last_seen: Instant,
    busy: bool,
}

/// Pending frame being assembled
struct PendingFrame {
    width: u32,
    height: u32,
    strips: HashMap<u32, Vec<u8>>,  // y_start -> pixel data
    expected_strips: usize,
    start_time: Instant,
    response_tx: oneshot::Sender<FrameResponse>,
}

/// Coordinator state
pub struct Coordinator {
    workers: RwLock<HashMap<String, WorkerInfo>>,
    pending_frames: RwLock<HashMap<u64, PendingFrame>>,
    next_frame_id: RwLock<u64>,
    frames_rendered: RwLock<u64>,
}

impl Coordinator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            workers: RwLock::new(HashMap::new()),
            pending_frames: RwLock::new(HashMap::new()),
            next_frame_id: RwLock::new(0),
            frames_rendered: RwLock::new(0),
        })
    }

    /// Start the profiling loop
    pub fn start_profile_loop(self: &Arc<Self>) {
        let coordinator = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(PROFILE_INTERVAL_SECS));
            loop {
                interval.tick().await;
                coordinator.run_profiling().await;
                coordinator.cleanup_stale_workers();
            }
        });
    }

    /// Run profiling on all workers
    async fn run_profiling(&self) {
        // Collect senders while holding lock, then release before awaiting
        let senders: Vec<mpsc::Sender<CoordinatorToWorker>> = {
            let workers = self.workers.read().unwrap();
            tracing::info!("Running profiling on {} workers", workers.len());
            workers.values().map(|w| w.sender.clone()).collect()
        };

        let msg = CoordinatorToWorker::RunProfile {
            width: PROFILE_WIDTH,
            height: PROFILE_HEIGHT,
        };

        for sender in senders {
            let _ = sender.send(msg.clone()).await;
        }
    }

    /// Remove workers that haven't sent a heartbeat recently
    fn cleanup_stale_workers(&self) {
        let timeout = Duration::from_secs(WORKER_TIMEOUT_SECS);
        let mut workers = self.workers.write().unwrap();
        let before = workers.len();
        workers.retain(|id, info| {
            let alive = info.last_seen.elapsed() < timeout;
            if !alive {
                tracing::warn!("Removing stale worker: {}", id);
            }
            alive
        });
        let removed = before - workers.len();
        if removed > 0 {
            tracing::info!("Removed {} stale workers", removed);
        }
    }

    /// Handle a new worker connection
    pub async fn handle_worker_connection(self: &Arc<Self>, socket: WebSocket) {
        let (ws_sender, mut ws_receiver) = socket.split();
        let (tx, rx) = mpsc::channel::<CoordinatorToWorker>(32);

        // Spawn task to forward messages to WebSocket
        let ws_sender = Arc::new(tokio::sync::Mutex::new(ws_sender));
        let ws_sender_clone = Arc::clone(&ws_sender);
        tokio::spawn(async move {
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                let text = serde_json::to_string(&msg).unwrap();
                let mut sender = ws_sender_clone.lock().await;
                if sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        });

        let coordinator = Arc::clone(self);
        let mut worker_id: Option<String> = None;

        while let Some(msg) = ws_receiver.next().await {
            let msg = match msg {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Pong(data)).await;
                    continue;
                }
                _ => continue,
            };

            let parsed: WorkerToCoordinator = match serde_json::from_str(&msg) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Invalid worker message: {}", e);
                    continue;
                }
            };

            match parsed {
                WorkerToCoordinator::Register { worker_id: id } => {
                    tracing::info!("Worker registered: {}", id);
                    worker_id = Some(id.clone());

                    // Add worker to pool
                    {
                        let mut workers = coordinator.workers.write().unwrap();
                        workers.insert(id.clone(), WorkerInfo {
                            sender: tx.clone(),
                            capability: 1.0,  // Default until profiled
                            last_seen: Instant::now(),
                            busy: false,
                        });
                    }

                    // Send acknowledgement
                    let _ = tx.send(CoordinatorToWorker::Registered { worker_id: id.clone() }).await;

                    // Request initial profiling
                    let _ = tx.send(CoordinatorToWorker::RunProfile {
                        width: PROFILE_WIDTH,
                        height: PROFILE_HEIGHT,
                    }).await;
                }

                WorkerToCoordinator::Heartbeat { worker_id: id } => {
                    if let Some(worker) = coordinator.workers.write().unwrap().get_mut(&id) {
                        worker.last_seen = Instant::now();
                    }
                }

                WorkerToCoordinator::ProfileResult { worker_id: id, compute_ms } => {
                    tracing::info!("Worker {} profile: {}ms", id, compute_ms);
                    if let Some(worker) = coordinator.workers.write().unwrap().get_mut(&id) {
                        // Capability is inverse of time (higher = faster)
                        worker.capability = 1000.0 / (compute_ms.max(1) as f64);
                        worker.last_seen = Instant::now();
                    }
                }

                WorkerToCoordinator::StripResult(result) => {
                    coordinator.handle_strip_result(result).await;
                }
            }
        }

        // Worker disconnected - remove from pool
        if let Some(id) = worker_id {
            tracing::info!("Worker disconnected: {}", id);
            coordinator.workers.write().unwrap().remove(&id);
        }
    }

    /// Handle a completed strip from a worker
    async fn handle_strip_result(&self, result: StripResult) {
        // Mark worker as not busy
        if let Some(worker) = self.workers.write().unwrap().get_mut(&result.worker_id) {
            worker.busy = false;
            worker.last_seen = Instant::now();
        }

        // Decode the strip data
        let pixel_data = match base64::engine::general_purpose::STANDARD.decode(&result.data) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to decode strip data: {}", e);
                return;
            }
        };

        // Add to pending frame
        let mut pending = self.pending_frames.write().unwrap();
        if let Some(frame) = pending.get_mut(&result.frame_id) {
            frame.strips.insert(result.y_start, pixel_data);

            // Check if frame is complete
            if frame.strips.len() == frame.expected_strips {
                // Assemble the frame
                let assembled = self.assemble_frame(frame);
                let render_ms = frame.start_time.elapsed().as_millis() as u64;

                let response = FrameResponse {
                    frame_id: result.frame_id,
                    width: frame.width,
                    height: frame.height,
                    render_ms,
                    data: base64::engine::general_purpose::STANDARD.encode(&assembled),
                };

                // Send response (take ownership of response_tx)
                if let Some(frame) = pending.remove(&result.frame_id) {
                    let _ = frame.response_tx.send(response);
                    *self.frames_rendered.write().unwrap() += 1;
                }
            }
        }
    }

    /// Assemble strips into a complete frame
    fn assemble_frame(&self, frame: &PendingFrame) -> Vec<u8> {
        let mut assembled = vec![0u8; (frame.width * frame.height * 3) as usize];

        // Sort strips by y_start and copy into assembled buffer
        let mut sorted_strips: Vec<_> = frame.strips.iter().collect();
        sorted_strips.sort_by_key(|(y_start, _)| *y_start);

        for (y_start, data) in sorted_strips {
            let offset = (*y_start * frame.width * 3) as usize;
            let end = offset + data.len();
            if end <= assembled.len() {
                assembled[offset..end].copy_from_slice(data);
            }
        }

        assembled
    }

    /// Handle a client frame request
    pub async fn request_frame(&self, request: FrameRequest) -> Result<FrameResponse, String> {
        let frame_id = {
            let mut id = self.next_frame_id.write().unwrap();
            let current = *id;
            *id += 1;
            current
        };

        // Get available workers and their capabilities
        let workers: Vec<(String, f64, mpsc::Sender<CoordinatorToWorker>)> = {
            let workers = self.workers.read().unwrap();
            workers.iter()
                .filter(|(_, info)| !info.busy)
                .map(|(id, info)| (id.clone(), info.capability, info.sender.clone()))
                .collect()
        };

        if workers.is_empty() {
            return Err("No workers available".to_string());
        }

        // Calculate total capability for proportional distribution
        let total_capability: f64 = workers.iter().map(|(_, c, _)| c).sum();

        // Assign strips to workers proportionally
        let mut strip_assignments = Vec::new();
        let mut current_y = 0u32;

        for (i, (worker_id, capability, sender)) in workers.iter().enumerate() {
            let proportion = capability / total_capability;
            let strip_height = if i == workers.len() - 1 {
                // Last worker gets the remainder
                request.height - current_y
            } else {
                ((request.height as f64) * proportion).round() as u32
            };

            if strip_height > 0 && current_y < request.height {
                let y_end = (current_y + strip_height).min(request.height);
                strip_assignments.push((
                    worker_id.clone(),
                    sender.clone(),
                    current_y,
                    y_end,
                ));
                current_y = y_end;
            }
        }

        if strip_assignments.is_empty() {
            return Err("Failed to assign strips".to_string());
        }

        // Create pending frame
        let (response_tx, response_rx) = oneshot::channel();
        {
            let mut pending = self.pending_frames.write().unwrap();
            pending.insert(frame_id, PendingFrame {
                width: request.width,
                height: request.height,
                strips: HashMap::new(),
                expected_strips: strip_assignments.len(),
                start_time: Instant::now(),
                response_tx,
            });
        }

        // Mark workers as busy and send requests
        {
            let mut workers = self.workers.write().unwrap();
            for (worker_id, _, _, _) in &strip_assignments {
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.busy = true;
                }
            }
        }

        // Send render requests to workers
        for (worker_id, sender, y_start, y_end) in strip_assignments {
            let msg = CoordinatorToWorker::RenderStrip(RenderStripRequest {
                frame_id,
                width: request.width,
                y_start,
                y_end,
                total_height: request.height,
                center_x: request.center_x,
                center_y: request.center_y,
                zoom: request.zoom,
                max_iterations: request.max_iterations,
            });

            if let Err(e) = sender.send(msg).await {
                tracing::error!("Failed to send to worker {}: {}", worker_id, e);
            }
        }

        // Wait for response with timeout
        match tokio::time::timeout(Duration::from_secs(30), response_rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err("Frame assembly cancelled".to_string()),
            Err(_) => {
                // Timeout - clean up pending frame
                self.pending_frames.write().unwrap().remove(&frame_id);
                Err("Frame render timeout".to_string())
            }
        }
    }

    /// Get current status
    pub fn get_status(&self) -> StatusResponse {
        let workers = self.workers.read().unwrap();
        let worker_statuses: Vec<WorkerStatus> = workers.iter()
            .map(|(id, info)| WorkerStatus {
                worker_id: id.clone(),
                capability: info.capability,
                last_seen_ms: info.last_seen.elapsed().as_millis() as u64,
            })
            .collect();

        StatusResponse {
            workers: worker_statuses,
            frames_rendered: *self.frames_rendered.read().unwrap(),
        }
    }

    /// Handle a client WebSocket connection
    pub async fn handle_client_connection(self: &Arc<Self>, socket: WebSocket) {
        let (mut sender, mut receiver) = socket.split();

        tracing::info!("Client connected");

        while let Some(msg) = receiver.next().await {
            let msg = match msg {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    let _ = sender.send(Message::Pong(data)).await;
                    continue;
                }
                _ => continue,
            };

            let parsed: ClientToCoordinator = match serde_json::from_str(&msg) {
                Ok(m) => m,
                Err(e) => {
                    let error = CoordinatorToClient::Error {
                        message: format!("Invalid message: {}", e),
                    };
                    let _ = sender.send(Message::Text(serde_json::to_string(&error).unwrap().into())).await;
                    continue;
                }
            };

            let response = match parsed {
                ClientToCoordinator::RequestFrame(req) => {
                    match self.request_frame(req).await {
                        Ok(frame) => CoordinatorToClient::Frame(frame),
                        Err(e) => CoordinatorToClient::Error { message: e },
                    }
                }
                ClientToCoordinator::GetStatus => {
                    CoordinatorToClient::Status(self.get_status())
                }
            };

            let text = serde_json::to_string(&response).unwrap();
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }

        tracing::info!("Client disconnected");
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self {
            workers: RwLock::new(HashMap::new()),
            pending_frames: RwLock::new(HashMap::new()),
            next_frame_id: RwLock::new(0),
            frames_rendered: RwLock::new(0),
        }
    }
}
