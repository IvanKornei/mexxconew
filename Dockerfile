# Multi-stage build for minimal image size
FROM rust:1.84-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev

WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Create dummy main to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM alpine:latest

RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/arbitrage-system /app/arbitrage-system

# Copy config template
COPY config.toml /app/config.toml

# Create logs directory
RUN mkdir -p /app/logs

EXPOSE 3000

CMD ["/app/arbitrage-system"]
