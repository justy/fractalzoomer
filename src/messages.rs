/// Shared message types for coordinator-worker and client-coordinator communication

use serde::{Deserialize, Serialize};

// ============================================================================
// Worker <-> Coordinator messages
// ============================================================================

/// Messages from worker to coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum WorkerToCoordinator {
    /// Worker registration
    Register { worker_id: String },
    /// Heartbeat
    Heartbeat { worker_id: String },
    /// Profile result
    ProfileResult { worker_id: String, compute_ms: u64 },
    /// Rendered strip result
    StripResult(StripResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripResult {
    pub worker_id: String,
    pub frame_id: u64,
    pub y_start: u32,
    pub y_end: u32,
    pub compute_ms: u64,
    pub data: String, // Base64 encoded RGB
}

/// Messages from coordinator to worker
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorToWorker {
    /// Acknowledgement of registration
    Registered { worker_id: String },
    /// Request to run profiling
    RunProfile { width: u32, height: u32 },
    /// Request to render a strip
    RenderStrip(RenderStripRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderStripRequest {
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

// ============================================================================
// Client <-> Coordinator messages
// ============================================================================

/// Messages from client to coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ClientToCoordinator {
    /// Request a frame
    RequestFrame(FrameRequest),
    /// Request current status
    GetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameRequest {
    pub width: u32,
    pub height: u32,
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub max_iterations: u32,
}

/// Messages from coordinator to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorToClient {
    /// Complete rendered frame
    Frame(FrameResponse),
    /// Status update
    Status(StatusResponse),
    /// Error
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameResponse {
    pub frame_id: u64,
    pub width: u32,
    pub height: u32,
    pub render_ms: u64,
    pub data: String, // Base64 encoded RGB
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub workers: Vec<WorkerStatus>,
    pub frames_rendered: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub worker_id: String,
    pub capability: f64,
    pub last_seen_ms: u64,
}
