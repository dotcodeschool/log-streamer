use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_stream::StreamExt;
use tokio::io::{self, AsyncReadExt};
use redis::{AsyncCommands, RedisError};
use mobc::{Pool};
use mobc_redis::{RedisConnectionManager, redis};
use log::{info, error};
use env_logger::init as env_logger_init;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger_init();

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    let mut incoming = TcpListenerStream::new(listener);
    info!("Listening on: 0.0.0.0:8080");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let client = redis::Client::open(redis_url).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let manager = RedisConnectionManager::new(client);
    let pool = Pool::builder().build(manager);

    while let Some(Ok(socket)) = incoming.next().await {
        let pool = pool.clone();
        tokio::spawn(handle_connection(socket, pool));
    }

    Ok(())
}

async fn handle_connection(mut socket: tokio::net::TcpStream, pool: Pool<RedisConnectionManager>) -> io::Result<()> {
    let mut buf = vec![0; 1024]; // Dynamic buffer with initial capacity
    loop {
        let n = socket.read(&mut buf).await?;
        if n == 0 { break; }

        let message = String::from_utf8_lossy(&buf[..n]).to_string();
        info!("Received: {}", message);

        match add_to_redis(&message, &pool).await {
            Ok(_) => info!("Pushed to Redis"),
            Err(e) => error!("Failed to add to Redis: {}", e),
        }
    }
    Ok(())
}

async fn add_to_redis(message: &str, pool: &Pool<RedisConnectionManager>) -> Result<(), RedisError> {
    let mut conn = pool.get().await.map_err(|e| RedisError::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    conn.xadd("log_stream", "*", &[("msg", message)]).await
}
