use anyhow::Result;

mod binance_client;
mod coinbase_client;
mod config;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Neo Oracle MVP starting");

    // Load configuration
    let settings = match config::Settings::load() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            // Depending on severity, you might want to panic or exit
            return Err(anyhow::anyhow!("Configuration loading failed: {}", e));
        }
    };
    log::info!("Configuration loaded successfully.");

    // Fetch and print Binance prices using config
    match binance_client::get_binance_prices(&settings.apis.binance).await {
        Ok(prices) => {
            log::info!("Successfully fetched prices from Binance:");
            for (symbol, price) in &prices {
                println!("Binance - {}: {}", symbol, price);
            }
        }
        Err(e) => {
            log::error!("Failed to fetch prices from Binance: {}", e);
        }
    }

    // Fetch and print Coinbase prices using config
    match coinbase_client::get_coinbase_prices(&settings.apis.coinbase).await {
        Ok(prices) => {
            log::info!("Successfully fetched prices from Coinbase:");
            for (symbol, price) in &prices {
                println!("Coinbase - {}: {}", symbol, price);
            }
        }
        Err(e) => {
            log::error!("Failed to fetch prices from Coinbase: {}", e);
        }
    }

    log::info!("Neo Oracle MVP finished");
    Ok(())
}
