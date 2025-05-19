use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct BinanceTickerResponse {
    pub symbol: String,
    pub price: String,
}

async fn get_binance_ticker_price(client: &Client, symbol: &str) -> Result<BinanceTickerResponse> {
    let url = format!("https://api.binance.com/api/v3/ticker/price?symbol={}", symbol);
    log::debug!("Fetching price for {} from Binance: {}", symbol, url);
    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?; // Ensure we have a success status
    let ticker_response = response.json::<BinanceTickerResponse>().await?;
    log::info!("Fetched price for {}: {}", symbol, ticker_response.price);
    Ok(ticker_response)
}

pub async fn get_binance_prices() -> Result<HashMap<String, String>> {
    let client = Client::new();
    let mut prices = HashMap::new();

    let symbols = ["BTCUSDT", "ETHUSDT"];

    for symbol in symbols.iter() {
        match get_binance_ticker_price(&client, symbol).await {
            Ok(ticker) => {
                prices.insert(ticker.symbol.clone(), ticker.price.clone());
            }
            Err(e) => {
                log::error!("Failed to fetch price for {}: {}", symbol, e);
                // Decide if we want to return an error for the whole function
                // or just skip this symbol. For MVP, let's log and continue,
                // but a more robust solution might retry or error out.
                // For now, to ensure we know something went wrong, let's propagate the first error.
                return Err(anyhow::anyhow!("Failed to fetch price for {}: {}", symbol, e));
            }
        }
    }

    if prices.len() < symbols.len() {
        log::warn!("Could not fetch prices for all requested symbols from Binance.");
    }

    Ok(prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_parse_binance_response() {
        let json_response = r#"
        {
            "symbol": "BTCUSDT",
            "price": "60000.00000000"
        }
        "#;
        let parsed: Result<BinanceTickerResponse, _> = serde_json::from_str(json_response);
        assert!(parsed.is_ok());
        let ticker = parsed.unwrap();
        assert_eq!(ticker.symbol, "BTCUSDT");
        assert_eq!(ticker.price, "60000.00000000");
    }

     #[test]
    fn test_parse_malformed_binance_response() {
        let json_response = r#"
        {
            "sym": "BTCUSDT", // incorrect field name
            "prc": "60000.00" // incorrect field name
        }
        "#;
        let parsed: Result<BinanceTickerResponse, _> = serde_json::from_str(json_response);
        assert!(parsed.is_err());
    }
} 