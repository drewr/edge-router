use anyhow::Result;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    println!("router-gateway starting...");
    
    // TODO: Implement router gateway
    Ok(())
}
