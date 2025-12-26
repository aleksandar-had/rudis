mod command;
mod resp;
mod server;

use anyhow::Result;
use server::Server;

#[tokio::main]
async fn main() -> Result<()> {
    let server = Server::new().await?;
    server.run().await?;
    Ok(())
}
