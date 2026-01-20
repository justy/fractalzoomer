# Build stage
FROM rust:1.83-bookworm AS builder

WORKDIR /app

# Copy manifest files first for better caching
COPY Cargo.toml Cargo.lock* ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the actual application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/fractal-zoomer .

# Copy static files
COPY static ./static

# Expose the default port
EXPOSE 8080

# Set environment variables
ENV RUST_LOG=info
ENV PORT=8080

# Run the binary
CMD ["./fractal-zoomer"]
