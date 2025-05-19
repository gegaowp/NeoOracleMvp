use anyhow::Result;
use tokio::time::{Duration, sleep};

mod aggregator;
mod binance_client;
mod coinbase_client;
mod config;
mod sui_publisher;

// Helper function to parse price string to Option<f64>
fn parse_price(price_str_opt: Option<&String>) -> Option<f64> {
    price_str_opt.and_then(|price_str| price_str.parse::<f64>().ok())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Neo Oracle MVP starting");

    let settings = match config::Settings::load() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            return Err(anyhow::anyhow!("Configuration loading failed: {}", e));
        }
    };
    log::info!("Configuration loaded successfully. Starting main loop...");

    loop {
        log::info!("--- Fetching new prices ---");

        let binance_prices_map =
            match binance_client::get_binance_prices(&settings.apis.binance).await {
                Ok(prices) => {
                    log::info!("Successfully fetched prices from Binance:");
                    for (symbol, price) in &prices {
                        log::debug!("Binance - {}: {}", symbol, price);
                    }
                    Some(prices)
                }
                Err(e) => {
                    log::error!("Failed to fetch prices from Binance: {}", e);
                    None
                }
            };

        let coinbase_prices_map =
            match coinbase_client::get_coinbase_prices(&settings.apis.coinbase).await {
                Ok(prices) => {
                    log::info!("Successfully fetched prices from Coinbase:");
                    for (symbol, price) in &prices {
                        log::debug!("Coinbase - {}: {}", symbol, price);
                    }
                    Some(prices)
                }
                Err(e) => {
                    log::error!("Failed to fetch prices from Coinbase: {}", e);
                    None
                }
            };

        let btc_binance_symbol = settings
            .apis
            .binance
            .symbols
            .iter()
            .find(|s| s.contains("BTC"))
            .map(|s| s.as_str());
        let btc_coinbase_symbol = settings
            .apis
            .coinbase
            .symbols
            .iter()
            .find(|s| s.contains("BTC"))
            .map(|s| s.as_str());
        let btc_price_binance = btc_binance_symbol.and_then(|sym| {
            binance_prices_map
                .as_ref()
                .and_then(|m| parse_price(m.get(sym)))
        });
        let btc_price_coinbase = btc_coinbase_symbol.and_then(|sym| {
            coinbase_prices_map
                .as_ref()
                .and_then(|m| parse_price(m.get(sym)))
        });

        let btc_prices_to_aggregate = [btc_price_binance, btc_price_coinbase];
        if let Some(aggregated_btc_price) = aggregator::aggregate_prices(&btc_prices_to_aggregate) {
            log::info!("Aggregated BTC/USD Price: {:.2}", aggregated_btc_price);
            let btc_price_info = sui_publisher::PriceInfo {
                symbol: "BTC/USD".to_string(), // Standardized symbol for on-chain
                price: aggregated_btc_price,
                timestamp_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            };
            match sui_publisher::submit_price_update(btc_price_info).await {
                Ok(digest) => log::info!(
                    "Successfully submitted BTC/USD price update to Sui. Digest: {}",
                    digest
                ),
                Err(e) => log::error!("Failed to submit BTC/USD price update to Sui: {:?}", e),
            }
        } else {
            log::warn!("Could not aggregate BTC/USD price. Not enough data.");
        }

        let eth_binance_symbol = settings
            .apis
            .binance
            .symbols
            .iter()
            .find(|s| s.contains("ETH"))
            .map(|s| s.as_str());
        let eth_coinbase_symbol = settings
            .apis
            .coinbase
            .symbols
            .iter()
            .find(|s| s.contains("ETH"))
            .map(|s| s.as_str());
        let eth_price_binance = eth_binance_symbol.and_then(|sym| {
            binance_prices_map
                .as_ref()
                .and_then(|m| parse_price(m.get(sym)))
        });
        let eth_price_coinbase = eth_coinbase_symbol.and_then(|sym| {
            coinbase_prices_map
                .as_ref()
                .and_then(|m| parse_price(m.get(sym)))
        });

        let eth_prices_to_aggregate = [eth_price_binance, eth_price_coinbase];
        if let Some(aggregated_eth_price) = aggregator::aggregate_prices(&eth_prices_to_aggregate) {
            log::info!("Aggregated ETH/USD Price: {:.2}", aggregated_eth_price);
            let eth_price_info = sui_publisher::PriceInfo {
                symbol: "ETH/USD".to_string(), // Standardized symbol for on-chain
                price: aggregated_eth_price,
                timestamp_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            };
            match sui_publisher::submit_price_update(eth_price_info).await {
                Ok(digest) => log::info!(
                    "Successfully submitted ETH/USD price update to Sui. Digest: {}",
                    digest
                ),
                Err(e) => log::error!("Failed to submit ETH/USD price update to Sui: {:?}", e),
            }
        } else {
            log::warn!("Could not aggregate ETH/USD price. Not enough data.");
        }

        log::info!(
            "--- Waiting for next fetch cycle ({} seconds) ---",
            settings.general.fetch_interval_seconds
        );
        sleep(Duration::from_secs(settings.general.fetch_interval_seconds)).await;
    }
}
