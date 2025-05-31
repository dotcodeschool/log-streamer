# Build stage
FROM rust:1.81-slim AS builder
WORKDIR /usr/src/app
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Create a new Cargo project if Cargo.toml doesn't exist
RUN cargo new --bin log-streamer
WORKDIR /usr/src/app/log-streamer
COPY . .

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/log-streamer/target/release/log-streamer /usr/local/bin/log-streamer
EXPOSE 3000
CMD ["log-streamer"] 