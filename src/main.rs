use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Neo Oracle MVP starting");
    println!("Hello, Neo Oracle MVP!");
    log::info!("Neo Oracle MVP finished");
    Ok(())
}
