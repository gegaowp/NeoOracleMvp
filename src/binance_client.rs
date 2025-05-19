use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use crate::config::ExchangeConfig;

#[derive(Deserialize, Debug)]
pub struct BinanceTickerResponse {
    pub symbol: String,
    pub price: String,
}

async fn get_binance_ticker_price(client: &Client, base_url: &str, symbol: &str) -> Result<BinanceTickerResponse> {
    let url = format!("{}?symbol={}", base_url, symbol);
    log::debug!("Fetching price for {} from Binance: {}", symbol, url);
    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?;
    let ticker_response = response.json::<BinanceTickerResponse>().await?;
    log::info!("Fetched price for {}: {}", symbol, ticker_response.price);
    Ok(ticker_response)
}

pub async fn get_binance_prices(config: &ExchangeConfig) -> Result<HashMap<String, String>> {
    let client = Client::new();
    let mut prices = HashMap::new();

    for symbol in &config.symbols {
        match get_binance_ticker_price(&client, &config.base_url, symbol).await {
            Ok(response) => {
                prices.insert(response.symbol.clone(), response.price);
            }
            Err(e) => {
                log::error!("Failed to fetch price for {} from Binance: {}", symbol, e);
            }
        }
    }
    Ok(prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_parse_binance_response() {
        let json_data = r#"{"symbol":"BTCUSDT","price":"60000.00"}"#;
        let parsed: Result<BinanceTickerResponse, _> = serde_json::from_str(json_data);
        assert!(parsed.is_ok());
        let response = parsed.unwrap();
        assert_eq!(response.symbol, "BTCUSDT");
        assert_eq!(response.price, "60000.00");
    }

    #[test]
    fn test_parse_malformed_binance_response() {
        let json_data = r#"{"symbol":"ETHUSDT"}"#; // Missing price
        let parsed: Result<BinanceTickerResponse, _> = serde_json::from_str(json_data);
        assert!(parsed.is_err());
    }

    // Example of how a test for get_binance_prices might look with mock HTTP server
    // This requires a mock library like wiremock or similar and is more involved.
    // For now, we are focusing on parsing tests.
    /*
    use crate::config::ExchangeConfig;
    #[tokio::test]
    async fn test_fetch_binance_prices_mocked() {
        // Setup mock server here to respond to base_url + ?symbol=...
        let mock_config = ExchangeConfig {
            base_url: "http://localhost:1234/mock_binance".to_string(), // Mock server URL
            symbols: vec!["BTCUSDT".to_string()],
        };
        // let prices = get_binance_prices(&mock_config).await.unwrap();
        // assert_eq!(prices.get("BTCUSDT"), Some(&"mock_price".to_string()));
    }
    */
} 