use clobber;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    clobber::go(8).await?;
    
    Ok(())
}