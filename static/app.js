/**
 * Fractal Zoomer Client
 *
 * Manages multiple WebSocket connections to worker servers,
 * distributes rendering work based on worker capability,
 * and assembles strips into complete frames.
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
        this.targetFps = parseInt(params.get('fps')) || 30;

        this.canvas.width = this.width;
        this.canvas.height = this.height;

        // Elephant Valley - a detailed region of the Mandelbrot set
        this.centerX = -0.74364388703;
        this.centerY = 0.13182590421;
        this.zoom = 1.0;
        this.zoomSpeed = 1.02;
        this.maxIterations = 500;

        // Worker management
        this.workers = [];
        this.workerCapabilities = new Map(); // worker index -> capability score
        this.targetWorkerCount = 4;

        // Frame management
        this.frameId = 0;
        this.pendingStrips = new Map(); // frame_id -> { strips: Map, expected: number }
        this.running = false;

        // FPS tracking
        this.frameTimestamps = [];
        this.lastStatsUpdate = 0;

        // Get worker URL(s) from params or use current host
        const workerUrls = params.get('workers');
        if (workerUrls) {
            this.workerBaseUrls = workerUrls.split(',');
        } else {
            // Default: connect to same host
            const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            this.workerBaseUrls = [`${wsProtocol}//${window.location.host}/ws`];
        }

        // UI elements
        this.fpsDisplay = document.getElementById('fps');
        this.zoomDisplay = document.getElementById('zoom');
        this.workersDisplay = document.getElementById('workers');
        this.frameDisplay = document.getElementById('frame');
        this.startBtn = document.getElementById('startBtn');
        this.stopBtn = document.getElementById('stopBtn');
        this.workerCountInput = document.getElementById('workerCount');
        this.maxIterInput = document.getElementById('maxIter');
        this.zoomSpeedInput = document.getElementById('zoomSpeed');

        this.setupEventListeners();
    }

    setupEventListeners() {
        this.startBtn.addEventListener('click', () => this.start());
        this.stopBtn.addEventListener('click', () => this.stop());

        this.workerCountInput.addEventListener('change', (e) => {
            this.targetWorkerCount = parseInt(e.target.value) || 4;
        });

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
        this.targetWorkerCount = parseInt(this.workerCountInput.value) || 4;
        this.maxIterations = parseInt(this.maxIterInput.value) || 500;
        this.zoomSpeed = parseFloat(this.zoomSpeedInput.value) || 1.02;

        // Connect to workers
        await this.connectWorkers();

        // Start rendering loop
        this.renderLoop();
    }

    stop() {
        this.running = false;
        this.startBtn.disabled = false;
        this.stopBtn.disabled = true;

        // Close all WebSocket connections
        for (const worker of this.workers) {
            if (worker.socket.readyState === WebSocket.OPEN) {
                worker.socket.close();
            }
        }
        this.workers = [];
        this.workerCapabilities.clear();
    }

    async connectWorkers() {
        const connectPromises = [];

        for (let i = 0; i < this.targetWorkerCount; i++) {
            // Round-robin through available URLs
            const url = this.workerBaseUrls[i % this.workerBaseUrls.length];
            connectPromises.push(this.connectWorker(i, url));
        }

        await Promise.all(connectPromises);

        // Benchmark all workers
        await this.benchmarkWorkers();
    }

    connectWorker(index, url) {
        return new Promise((resolve, reject) => {
            const socket = new WebSocket(url);

            socket.onopen = () => {
                console.log(`Worker ${index} connected to ${url}`);
                this.workers[index] = {
                    socket,
                    url,
                    busy: false,
                    capability: 1.0
                };
                resolve();
            };

            socket.onmessage = (event) => {
                this.handleWorkerMessage(index, JSON.parse(event.data));
            };

            socket.onerror = (error) => {
                console.error(`Worker ${index} error:`, error);
            };

            socket.onclose = () => {
                console.log(`Worker ${index} disconnected`);
                if (this.running) {
                    // Attempt reconnection after delay
                    setTimeout(() => {
                        if (this.running) {
                            this.connectWorker(index, url);
                        }
                    }, 1000);
                }
            };

            // Timeout for connection
            setTimeout(() => {
                if (socket.readyState !== WebSocket.OPEN) {
                    socket.close();
                    reject(new Error(`Worker ${index} connection timeout`));
                }
            }, 5000);
        });
    }

    async benchmarkWorkers() {
        const benchmarkPromises = [];

        for (let i = 0; i < this.workers.length; i++) {
            if (this.workers[i] && this.workers[i].socket.readyState === WebSocket.OPEN) {
                benchmarkPromises.push(this.benchmarkWorker(i));
            }
        }

        await Promise.all(benchmarkPromises);

        // Normalise capabilities
        this.normaliseCapabilities();
    }

    benchmarkWorker(index) {
        return new Promise((resolve) => {
            const worker = this.workers[index];

            // Store callback for benchmark result
            worker.benchmarkResolve = resolve;

            // Send benchmark request
            worker.socket.send(JSON.stringify({
                type: 'benchmark',
                width: 256,
                height: 256
            }));

            // Timeout
            setTimeout(() => {
                if (worker.benchmarkResolve) {
                    worker.capability = 0.1; // Low score for timeout
                    worker.benchmarkResolve();
                    delete worker.benchmarkResolve;
                }
            }, 10000);
        });
    }

    normaliseCapabilities() {
        // Convert times to capabilities (inverse of time)
        // Higher capability = more pixels per second
        let totalCapability = 0;

        for (let i = 0; i < this.workers.length; i++) {
            if (this.workers[i]) {
                totalCapability += this.workers[i].capability;
            }
        }

        // Normalise so they sum to 1
        for (let i = 0; i < this.workers.length; i++) {
            if (this.workers[i]) {
                this.workers[i].capability /= totalCapability;
            }
        }

        console.log('Worker capabilities:', this.workers.map((w, i) =>
            w ? `${i}: ${(w.capability * 100).toFixed(1)}%` : null
        ).filter(Boolean));
    }

    handleWorkerMessage(workerIndex, message) {
        const worker = this.workers[workerIndex];
        if (!worker) return;

        switch (message.type) {
            case 'benchmark_result':
                // Convert compute time to capability score (inverse)
                worker.capability = 1000 / (message.compute_ms || 1);
                if (worker.benchmarkResolve) {
                    worker.benchmarkResolve();
                    delete worker.benchmarkResolve;
                }
                break;

            case 'strip':
                this.handleStripResult(workerIndex, message);
                break;

            case 'error':
                console.error(`Worker ${workerIndex} error:`, message.message);
                worker.busy = false;
                break;
        }
    }

    handleStripResult(workerIndex, message) {
        const worker = this.workers[workerIndex];
        if (worker) {
            worker.busy = false;
        }

        const frameData = this.pendingStrips.get(message.frame_id);
        if (!frameData) {
            // Frame already completed or discarded
            return;
        }

        // Store the strip
        frameData.strips.set(message.y_start, {
            yStart: message.y_start,
            yEnd: message.y_end,
            data: message.data
        });

        // Check if frame is complete
        if (frameData.strips.size === frameData.expected) {
            this.assembleFrame(message.frame_id, frameData);
            this.pendingStrips.delete(message.frame_id);
        }
    }

    assembleFrame(frameId, frameData) {
        // Sort strips by y position
        const sortedStrips = Array.from(frameData.strips.values())
            .sort((a, b) => a.yStart - b.yStart);

        // Create ImageData for the full frame
        const imageData = this.ctx.createImageData(this.width, this.height);

        for (const strip of sortedStrips) {
            // Decode base64 RGB data
            const binaryString = atob(strip.data);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
                bytes[i] = binaryString.charCodeAt(i);
            }

            // Copy RGB to RGBA ImageData
            const stripHeight = strip.yEnd - strip.yStart;
            for (let y = 0; y < stripHeight; y++) {
                for (let x = 0; x < this.width; x++) {
                    const srcIdx = (y * this.width + x) * 3;
                    const dstIdx = ((strip.yStart + y) * this.width + x) * 4;

                    imageData.data[dstIdx] = bytes[srcIdx];         // R
                    imageData.data[dstIdx + 1] = bytes[srcIdx + 1]; // G
                    imageData.data[dstIdx + 2] = bytes[srcIdx + 2]; // B
                    imageData.data[dstIdx + 3] = 255;               // A
                }
            }
        }

        // Draw to canvas
        this.ctx.putImageData(imageData, 0, 0);

        // Update FPS
        this.frameTimestamps.push(performance.now());
        this.updateStats();
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

        // Count connected workers
        const connectedWorkers = this.workers.filter(w =>
            w && w.socket.readyState === WebSocket.OPEN
        ).length;

        // Update displays
        this.fpsDisplay.textContent = `FPS: ${fps}`;
        this.zoomDisplay.textContent = `Zoom: ${this.formatZoom(this.zoom)}`;
        this.workersDisplay.textContent = `Workers: ${connectedWorkers}/${this.targetWorkerCount}`;
        this.frameDisplay.textContent = `Frame: ${this.frameId}`;
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

        // Check if we can start a new frame
        // Only allow 2 frames in flight to avoid memory buildup
        if (this.pendingStrips.size < 2) {
            this.requestFrame();
        }

        // Continue loop
        requestAnimationFrame(() => this.renderLoop());
    }

    requestFrame() {
        const availableWorkers = this.workers.filter(w =>
            w && !w.busy && w.socket.readyState === WebSocket.OPEN
        );

        if (availableWorkers.length === 0) return;

        const frameId = this.frameId++;

        // Calculate strip assignments based on capability
        const assignments = this.calculateStripAssignments(availableWorkers);

        // Create frame tracking
        this.pendingStrips.set(frameId, {
            strips: new Map(),
            expected: assignments.length
        });

        // Scale max iterations with zoom level
        const scaledIterations = Math.min(
            this.maxIterations + Math.floor(Math.log2(this.zoom) * 50),
            5000
        );

        // Send requests to workers
        for (const assignment of assignments) {
            const worker = assignment.worker;
            worker.busy = true;

            worker.socket.send(JSON.stringify({
                type: 'render',
                frame_id: frameId,
                width: this.width,
                y_start: assignment.yStart,
                y_end: assignment.yEnd,
                total_height: this.height,
                center_x: this.centerX,
                center_y: this.centerY,
                zoom: this.zoom,
                max_iterations: scaledIterations
            }));
        }

        // Increase zoom for next frame
        this.zoom *= this.zoomSpeed;
    }

    calculateStripAssignments(workers) {
        const assignments = [];
        let currentY = 0;

        // Calculate total capability
        const totalCapability = workers.reduce((sum, w) => sum + w.capability, 0);

        for (let i = 0; i < workers.length; i++) {
            const worker = workers[i];
            const proportion = worker.capability / totalCapability;

            // Calculate strip height based on capability proportion
            let stripHeight;
            if (i === workers.length - 1) {
                // Last worker gets the remainder
                stripHeight = this.height - currentY;
            } else {
                stripHeight = Math.round(this.height * proportion);
            }

            // Ensure at least 1 pixel
            stripHeight = Math.max(1, stripHeight);

            // Don't exceed remaining height
            stripHeight = Math.min(stripHeight, this.height - currentY);

            if (stripHeight > 0) {
                assignments.push({
                    worker,
                    yStart: currentY,
                    yEnd: currentY + stripHeight
                });
                currentY += stripHeight;
            }
        }

        return assignments;
    }
}

// Initialise when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.zoomer = new FractalZoomer();
});
