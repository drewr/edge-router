use anyhow::Result;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    println!("tunnel-gateway starting...");
    
    // TODO: Implement tunnel gateway
    Ok(())
}
