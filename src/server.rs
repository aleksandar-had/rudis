use crate::command::Command;
use crate::resp::RespValue;
use crate::store::Store;
use anyhow::Result;
use bytes::{Buf, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const REDIS_PORT: u16 = 6379;

pub struct Server {
    listener: TcpListener,
    store: Store,
}

impl Server {
    /// Create a new Redis server
    pub async fn new() -> Result<Self> {
        let addr = format!("127.0.0.1:{}", REDIS_PORT);
        let listener = TcpListener::bind(&addr).await?;
        println!("Rudis server listening on {}", addr);
        Ok(Self {
            listener,
            store: Store::new(),
        })
    }

    /// Run the server, accepting connections and handling them
    pub async fn run(&self) -> Result<()> {
        loop {
            let (socket, addr) = self.listener.accept().await?;
            println!("Accepted connection from {}", addr);

            // Clone the store handle for this connection
            let store = self.store.clone();

            // Spawn a new task to handle this connection
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, store).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }
}

// Handle a single client connection
async fn handle_connection(mut socket: TcpStream, store: Store) -> Result<()> {
    let mut buffer = BytesMut::with_capacity(4096);

    loop {
        // Read data from the socket
        let n = socket.read_buf(&mut buffer).await?;

        if n == 0 {
            // Connection closed
            return Ok(());
        }

        // Try to parse RESP values from the buffer
        while !buffer.is_empty() {
            match RespValue::parse(&mut buffer)? {
                Some((value, consumed)) => {
                    // We got a complete RESP value
                    let response = match Command::from_resp(value) {
                        Ok(cmd) => cmd.execute(&store).await,
                        Err(e) => RespValue::Error(e.to_string()),
                    };

                    // Send the response
                    socket.write_all(&response.serialize()).await?;

                    // Remove the consumed bytes from the buffer
                    buffer.advance(consumed);
                }
                None => {
                    // Need more data, break and read more
                    break;
                }
            }
        }
    }
}
