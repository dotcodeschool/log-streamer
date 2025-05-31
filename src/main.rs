use env_logger::init as env_logger_init;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use log::{error, info};
use mobc::Pool;
use mobc_redis::{redis, RedisConnectionManager};
use redis::{AsyncCommands, RedisError};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "event_type", rename_all = "lowercase")]
enum ClientMessage {
    Init { stream_id: String },
    Log { bytes: String },
    Disconnect,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger_init();

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    info!("Listening on: 0.0.0.0:8080");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    info!("Connecting to Redis at: {}", redis_url);

    let client =
        redis::Client::open(redis_url).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let manager = RedisConnectionManager::new(client);
    let pool = Pool::builder().build(manager);

    // Test Redis connection by trying to write to a test stream
    let mut conn = pool.get().await.map_err(|e| {
        error!("Failed to get Redis connection: {}", e);
        io::Error::new(io::ErrorKind::Other, e)
    })?;

    // Test connection using XADD
    let test_result: Result<String, RedisError> = conn
        .xadd("connection_test", "*", &[("event", "ping")])
        .await;

    match test_result {
        Ok(_) => info!("Redis connection test successful"),
        Err(e) => error!("Redis connection test failed: {}", e),
    }

    while let Ok((stream, _)) = listener.accept().await {
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, pool).await {
                error!("Error in connection handler: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    stream: TcpStream,
    pool: Pool<RedisConnectionManager>,
) -> io::Result<()> {
    let ws_stream = accept_async(stream).await.map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("WebSocket handshake failed: {}", e),
        )
    })?;
    let (mut write, mut read) = ws_stream.split();
    let mut stream_id = String::new();

    while let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let msg = msg
            .into_text()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        info!("Received message: {}", msg);

        match serde_json::from_str::<ClientMessage>(&msg) {
            Ok(ClientMessage::Init { stream_id: sid }) => {
                info!("Init message received with stream_id: {}", sid);
                stream_id = sid;
            }
            Ok(ClientMessage::Log { bytes }) => {
                if stream_id.is_empty() {
                    error!("Log message received before init");
                    write
                        .send(tungstenite::Message::Text("Error: No init".to_string()))
                        .await
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                } else if is_http_header(&bytes) {
                    info!("HTTP header detected, not logging");
                } else {
                    info!("Attempting to add log to Redis stream: {}", stream_id);
                    match add_to_redis(&stream_id, "log", &bytes, &pool).await {
                        Ok(_) => {
                            info!("Successfully added log to Redis stream");
                            write
                                .send(tungstenite::Message::Text("OK".to_string()))
                                .await
                                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                        }
                        Err(e) => {
                            error!("Failed to add log to Redis: {}", e);
                            write
                                .send(tungstenite::Message::Text(
                                    "Error: Redis write failed".to_string(),
                                ))
                                .await
                                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                        }
                    }
                }
            }
            Ok(ClientMessage::Disconnect) => {
                info!("Disconnect message received");
                if !stream_id.is_empty() {
                    match add_to_redis(&stream_id, "disconnect", "", &pool).await {
                        Ok(_) => info!("Successfully added disconnect event to Redis"),
                        Err(e) => error!("Failed to add disconnect event to Redis: {}", e),
                    }
                }
                break;
            }
            Err(e) => {
                error!("Error parsing message: {}", e);
                write
                    .send(tungstenite::Message::Text("Error".to_string()))
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
        }
    }
    Ok(())
}

async fn add_to_redis(
    stream_id: &str,
    event_type: &str,
    message: &str,
    pool: &Pool<RedisConnectionManager>,
) -> Result<(), RedisError> {
    let mut conn = pool.get().await.map_err(|e| {
        error!("Failed to get Redis connection in add_to_redis: {}", e);
        RedisError::from(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;

    info!(
        "Adding to Redis - Stream: {}, Event: {}",
        stream_id, event_type
    );
    let result = if event_type == "log" {
        conn.xadd(
            stream_id,
            "*",
            &[("event_type", event_type), ("bytes", message)],
        )
        .await
    } else {
        conn.xadd(stream_id, "*", &[("event_type", event_type)])
            .await
    };

    match &result {
        Ok(_) => info!("Successfully added event to Redis stream"),
        Err(e) => error!("Failed to add event to Redis stream: {}", e),
    }

    result
}

fn is_http_header(message: &str) -> bool {
    // Regular expression to match HTTP headers
    let re = Regex::new(r"(?i)^(GET|POST|PUT|DELETE|HEAD|OPTIONS|PATCH|TRACE|CONNECT) .* HTTP/\d\.\d\r\n(?:[^\r\n]+\r\n)*\r\n").unwrap();
    re.is_match(message)
}
