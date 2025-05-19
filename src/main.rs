use anyhow::Result;

mod aggregator;
mod binance_client;
mod coinbase_client;
mod config;

// Helper function to parse price string to Option<f64>
fn parse_price(price_str_opt: Option<&String>) -> Option<f64> {
    price_str_opt.and_then(|price_str| price_str.parse::<f64>().ok())
}

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

    let binance_prices_map = match binance_client::get_binance_prices(&settings.apis.binance).await {
        Ok(prices) => {
            log::info!("Successfully fetched prices from Binance:");
            for (symbol, price) in &prices {
                println!("Binance - {}: {}", symbol, price);
            }
            Some(prices)
        }
        Err(e) => {
            log::error!("Failed to fetch prices from Binance: {}", e);
            None
        }
    };

    let coinbase_prices_map = match coinbase_client::get_coinbase_prices(&settings.apis.coinbase).await {
        Ok(prices) => {
            log::info!("Successfully fetched prices from Coinbase:");
            for (symbol, price) in &prices {
                println!("Coinbase - {}: {}", symbol, price);
            }
            Some(prices)
        }
        Err(e) => {
            log::error!("Failed to fetch prices from Coinbase: {}", e);
            None
        }
    };

    // --- Price Aggregation ---
    // Define asset mappings from config symbols. 
    // For MVP, we assume fixed symbols from config, but this could be more dynamic.
    // Binance: BTCUSDT, ETHUSDT (from config.apis.binance.symbols)
    // Coinbase: BTC-USD, ETH-USD (from config.apis.coinbase.symbols)

    // Aggregate BTC price
    let btc_binance_symbol = settings.apis.binance.symbols.iter().find(|s| s.contains("BTC")).map(|s| s.as_str());
    let btc_coinbase_symbol = settings.apis.coinbase.symbols.iter().find(|s| s.contains("BTC")).map(|s| s.as_str());

    let btc_price_binance = btc_binance_symbol.and_then(|sym| binance_prices_map.as_ref().and_then(|m| parse_price(m.get(sym))));
    let btc_price_coinbase = btc_coinbase_symbol.and_then(|sym| coinbase_prices_map.as_ref().and_then(|m| parse_price(m.get(sym))));
    
    let btc_prices_to_aggregate = [btc_price_binance, btc_price_coinbase];
    if let Some(aggregated_btc_price) = aggregator::aggregate_prices(&btc_prices_to_aggregate) {
        log::info!("Aggregated BTC/USD Price: {:.2}", aggregated_btc_price);
        println!("Aggregated BTC/USD Price: {:.2}", aggregated_btc_price);
    } else {
        log::warn!("Could not aggregate BTC/USD price. Not enough data.");
        println!("Could not aggregate BTC/USD price. Not enough data.");
    }

    // Aggregate ETH price
    let eth_binance_symbol = settings.apis.binance.symbols.iter().find(|s| s.contains("ETH")).map(|s| s.as_str());
    let eth_coinbase_symbol = settings.apis.coinbase.symbols.iter().find(|s| s.contains("ETH")).map(|s| s.as_str());

    let eth_price_binance = eth_binance_symbol.and_then(|sym| binance_prices_map.as_ref().and_then(|m| parse_price(m.get(sym))));
    let eth_price_coinbase = eth_coinbase_symbol.and_then(|sym| coinbase_prices_map.as_ref().and_then(|m| parse_price(m.get(sym))));

    let eth_prices_to_aggregate = [eth_price_binance, eth_price_coinbase];
    if let Some(aggregated_eth_price) = aggregator::aggregate_prices(&eth_prices_to_aggregate) {
        log::info!("Aggregated ETH/USD Price: {:.2}", aggregated_eth_price);
        println!("Aggregated ETH/USD Price: {:.2}", aggregated_eth_price);
    } else {
        log::warn!("Could not aggregate ETH/USD price. Not enough data.");
        println!("Could not aggregate ETH/USD price. Not enough data.");
    }

    log::info!("Neo Oracle MVP finished");
    Ok(())
}
