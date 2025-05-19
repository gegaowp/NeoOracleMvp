use crate::config::ExchangeConfig;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct CoinbaseTickerResponse {
    pub price: String,
    // Coinbase API might return other fields like "trade_id", "size", "time", "bid", "ask", "volume"
    // We only care about the price for now.
}

async fn get_coinbase_ticker_price(
    client: &Client,
    base_url: &str,
    product_id: &str,
) -> Result<CoinbaseTickerResponse> {
    // Construct URL from base_url and product_id
    let url = format!("{}/{}/ticker", base_url, product_id);
    log::debug!("Fetching price for {} from Coinbase: {}", product_id, url);

    // Coinbase API often requires a User-Agent header
    let response = client
        .get(&url)
        .header("User-Agent", "neo-oracle-mvp") // Simple User-Agent
        .send()
        .await?;

    response.error_for_status_ref()?; // Ensure we have a success status
    let ticker_response = response.json::<CoinbaseTickerResponse>().await?;
    log::info!(
        "Fetched price for {}: {}",
        product_id,
        ticker_response.price
    );
    Ok(ticker_response)
}

pub async fn get_coinbase_prices(config: &ExchangeConfig) -> Result<HashMap<String, String>> {
    let client = Client::new();
    let mut prices = HashMap::new();

    // Use product_ids from config.symbols
    for product_id in &config.symbols {
        match get_coinbase_ticker_price(&client, &config.base_url, product_id).await {
            Ok(response) => {
                prices.insert(product_id.to_string(), response.price);
            }
            Err(e) => {
                log::error!(
                    "Failed to fetch price for {} from Coinbase: {}",
                    product_id,
                    e
                );
                // We can decide to return an error for the whole function or just skip this symbol
                // For MVP, let's log the error and continue, so one failure doesn't stop all.
                // If a more robust error handling is needed, we can change this.
                // Alternatively, to propagate the error: return Err(e.into());
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
    fn test_parse_coinbase_response() {
        let json_data = r#"
        {
            "trade_id": 4729088,
            "price": "30000.00",
            "size": "0.001",
            "time": "2023-10-27T10:00:00Z",
            "bid": "29999.00",
            "ask": "30001.00",
            "volume": "1000.0"
        }
        "#;
        let parsed: Result<CoinbaseTickerResponse, _> = serde_json::from_str(json_data);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().price, "30000.00");
    }

    #[test]
    fn test_parse_malformed_coinbase_response() {
        // Test with a missing price field
        let json_data_missing_price = r#"
        {
            "trade_id": 4729088,
            "size": "0.001"
        }
        "#;
        let parsed_missing: Result<CoinbaseTickerResponse, _> =
            serde_json::from_str(json_data_missing_price);
        assert!(parsed_missing.is_err());

        // Test with price as a number instead of string (if API guarantees string, this should fail)
        let json_data_wrong_type = r#"
        {
            "price": 30000.00
        }
        "#;
        let parsed_wrong_type: Result<CoinbaseTickerResponse, _> =
            serde_json::from_str(json_data_wrong_type);
        // serde might be flexible here, depending on the exact setup.
        // For a strict string requirement, this might pass or fail based on serde's deserialization leniency.
        // Let's assume for now it should be a string, so a direct number might cause issues if not handled.
        // If price can be number or string, the struct field type should be adjusted.
        // For now, we expect a string, so this should ideally be an error or handled appropriately.
        // Sticking to simple test: if it parses, ensure it's not what we expected if it's wrong type.
        // If it does parse a number as string, the value would be "30000" or "30000.0".
        // Let's assume strict string parsing.
        assert!(
            parsed_wrong_type.is_err(),
            "Price should be a string, not a number directly."
        );
    }
}
