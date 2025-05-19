use anyhow::Result;

mod binance_client;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Neo Oracle MVP starting");

    // Fetch and print Binance prices
    match binance_client::get_binance_prices().await {
        Ok(prices) => {
            log::info!("Successfully fetched prices from Binance:");
            for (symbol, price) in prices {
                println!("Binance - {}: {}", symbol, price);
            }
        }
        Err(e) => {
            log::error!("Failed to fetch prices from Binance: {}", e);
        }
    }

    println!("Hello, Neo Oracle MVP!");
    log::info!("Neo Oracle MVP finished");
    Ok(())
}
