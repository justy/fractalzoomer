/**
 * Fractal Zoomer Client
 *
 * Connects to the coordinator and requests complete frames.
 * The coordinator handles worker management and frame assembly.
 */

class FractalZoomer {
    constructor() {
        // Canvas setup
        this.canvas = document.getElementById('fractal');
        this.ctx = this.canvas.getContext('2d');

        // Get dimensions from URL params or defaults
        const params = new URLSearchParams(window.location.search);
        this.width = parseInt(params.get('w')) || 1280;
        this.height = parseInt(params.get('h')) || 720;

        this.canvas.width = this.width;
        this.canvas.height = this.height;

        // Elephant Valley - a detailed region of the Mandelbrot set
        this.centerX = -0.74364388703;
        this.centerY = 0.13182590421;
        this.zoom = 1.0;
        this.zoomSpeed = 1.02;
        this.maxIterations = 500;

        // Connection state
        this.socket = null;
        this.connected = false;
        this.running = false;
        this.pendingFrame = false;

        // Stats
        this.frameCount = 0;
        this.frameTimestamps = [];
        this.lastStatsUpdate = 0;
        this.lastRenderMs = 0;
        this.workerCount = 0;

        // UI elements
        this.fpsDisplay = document.getElementById('fps');
        this.zoomDisplay = document.getElementById('zoom');
        this.workersDisplay = document.getElementById('workers');
        this.frameDisplay = document.getElementById('frame');
        this.startBtn = document.getElementById('startBtn');
        this.stopBtn = document.getElementById('stopBtn');
        this.maxIterInput = document.getElementById('maxIter');
        this.zoomSpeedInput = document.getElementById('zoomSpeed');

        this.setupEventListeners();
    }

    setupEventListeners() {
        this.startBtn.addEventListener('click', () => this.start());
        this.stopBtn.addEventListener('click', () => this.stop());

        this.maxIterInput.addEventListener('change', (e) => {
            this.maxIterations = parseInt(e.target.value) || 500;
        });

        this.zoomSpeedInput.addEventListener('change', (e) => {
            this.zoomSpeed = parseFloat(e.target.value) || 1.02;
        });
    }

    async start() {
        this.running = true;
        this.startBtn.disabled = true;
        this.stopBtn.disabled = false;

        // Read current values from inputs
        this.maxIterations = parseInt(this.maxIterInput.value) || 500;
        this.zoomSpeed = parseFloat(this.zoomSpeedInput.value) || 1.02;

        // Connect to coordinator
        await this.connect();

        // Start rendering loop
        this.renderLoop();
    }

    stop() {
        this.running = false;
        this.startBtn.disabled = false;
        this.stopBtn.disabled = true;

        if (this.socket && this.socket.readyState === WebSocket.OPEN) {
            this.socket.close();
        }
        this.socket = null;
        this.connected = false;
    }

    connect() {
        return new Promise((resolve, reject) => {
            const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const url = `${wsProtocol}//${window.location.host}/ws/client`;

            console.log('Connecting to coordinator:', url);
            this.socket = new WebSocket(url);

            this.socket.onopen = () => {
                console.log('Connected to coordinator');
                this.connected = true;
                // Request initial status
                this.requestStatus();
                resolve();
            };

            this.socket.onmessage = (event) => {
                this.handleMessage(JSON.parse(event.data));
            };

            this.socket.onerror = (error) => {
                console.error('WebSocket error:', error);
            };

            this.socket.onclose = () => {
                console.log('Disconnected from coordinator');
                this.connected = false;
                if (this.running) {
                    // Attempt reconnection
                    setTimeout(() => {
                        if (this.running) {
                            this.connect();
                        }
                    }, 1000);
                }
            };

            // Timeout for connection
            setTimeout(() => {
                if (!this.connected) {
                    this.socket.close();
                    reject(new Error('Connection timeout'));
                }
            }, 5000);
        });
    }

    handleMessage(message) {
        switch (message.type) {
            case 'frame':
                this.handleFrame(message);
                break;
            case 'status':
                this.handleStatus(message);
                break;
            case 'error':
                console.error('Coordinator error:', message.message);
                this.pendingFrame = false;
                break;
        }
    }

    handleFrame(frame) {
        this.pendingFrame = false;
        this.lastRenderMs = frame.render_ms;

        // Decode base64 RGB data
        const binaryString = atob(frame.data);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }

        // Create ImageData and copy RGB to RGBA
        const imageData = this.ctx.createImageData(frame.width, frame.height);
        for (let i = 0, j = 0; i < bytes.length; i += 3, j += 4) {
            imageData.data[j] = bytes[i];         // R
            imageData.data[j + 1] = bytes[i + 1]; // G
            imageData.data[j + 2] = bytes[i + 2]; // B
            imageData.data[j + 3] = 255;          // A
        }

        // Draw to canvas
        this.ctx.putImageData(imageData, 0, 0);

        // Update stats
        this.frameCount++;
        this.frameTimestamps.push(performance.now());
        this.updateStats();

        // Increase zoom for next frame
        this.zoom *= this.zoomSpeed;
    }

    handleStatus(status) {
        this.workerCount = status.workers.length;
        this.updateStats();

        // Log worker capabilities
        if (status.workers.length > 0) {
            console.log('Workers:', status.workers.map(w =>
                `${w.worker_id.substring(0, 8)}: ${w.capability.toFixed(2)}`
            ).join(', '));
        }
    }

    requestStatus() {
        if (this.socket && this.socket.readyState === WebSocket.OPEN) {
            this.socket.send(JSON.stringify({ type: 'get_status' }));
        }
    }

    updateStats() {
        const now = performance.now();

        // Update at most 10 times per second
        if (now - this.lastStatsUpdate < 100) return;
        this.lastStatsUpdate = now;

        // Calculate FPS from recent frames
        const cutoff = now - 1000;
        this.frameTimestamps = this.frameTimestamps.filter(t => t > cutoff);
        const fps = this.frameTimestamps.length;

        // Update displays
        this.fpsDisplay.textContent = `FPS: ${fps} (${this.lastRenderMs}ms)`;
        this.zoomDisplay.textContent = `Zoom: ${this.formatZoom(this.zoom)}`;
        this.workersDisplay.textContent = `Workers: ${this.workerCount}`;
        this.frameDisplay.textContent = `Frame: ${this.frameCount}`;
    }

    formatZoom(zoom) {
        if (zoom >= 1e12) return (zoom / 1e12).toFixed(2) + 'T';
        if (zoom >= 1e9) return (zoom / 1e9).toFixed(2) + 'B';
        if (zoom >= 1e6) return (zoom / 1e6).toFixed(2) + 'M';
        if (zoom >= 1e3) return (zoom / 1e3).toFixed(2) + 'K';
        return zoom.toFixed(2) + 'x';
    }

    renderLoop() {
        if (!this.running) return;

        // Request next frame if not already pending
        if (!this.pendingFrame && this.connected) {
            this.requestFrame();
        }

        // Continue loop
        requestAnimationFrame(() => this.renderLoop());
    }

    requestFrame() {
        if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
            return;
        }

        this.pendingFrame = true;

        // Scale max iterations with zoom level
        const scaledIterations = Math.min(
            this.maxIterations + Math.floor(Math.log2(this.zoom) * 50),
            5000
        );

        const request = {
            type: 'request_frame',
            width: this.width,
            height: this.height,
            center_x: this.centerX,
            center_y: this.centerY,
            zoom: this.zoom,
            max_iterations: scaledIterations
        };

        this.socket.send(JSON.stringify(request));

        // Periodically request status
        if (this.frameCount % 60 === 0) {
            this.requestStatus();
        }
    }
}

// Initialise when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.zoomer = new FractalZoomer();
});
