> [!CAUTION]
> **This repository has been archived and is no longer maintained.**
> 
> This log streaming service was part of an older architecture where test results were streamed to students via Redis and WebSocket. Dot Code School now uses a fully static pipeline with no backend services.
> 
> See [dotcodeschool/frontend](https://github.com/dotcodeschool/frontend) for the current platform.

---

# Dot Code School Log Streamer

A Rust-based service that streams logs from Redis to connected clients via WebSocket.

## Prerequisites

- Rust toolchain (latest stable)
- Docker (for containerized deployment)

## Development

### Building

```bash
# Build the project
cargo build

# Run in development mode
cargo run
```

### Testing

```bash
# Run tests
cargo test
```

## Configuration

The service is configured through environment variables:

```bash
# Redis connection
REDIS_URL=redis://default:password@redis:6379

# WebSocket server
WS_PORT=8080  # Port to listen on for WebSocket connections
```

## Docker Deployment

Build and run using Docker:

```bash
# Build the image
docker build -t dcs-log-streamer .

# Run the container
docker run -d \
  -p 8080:8080 \
  -e REDIS_URL=redis://default:password@redis:6379 \
  dcs-log-streamer
```

## Architecture

The log streamer:

1. Connects to Redis and subscribes to log channels
2. Listens for WebSocket connections from clients
3. Forwards logs from Redis to connected clients in real-time
4. Handles client disconnections and reconnections gracefully

## License

This project is licensed under the [WTFPL](LICENSE) - Do What The Fuck You Want To Public License.
