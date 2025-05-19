use anyhow::Result;
use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ExchangeConfig {
    pub base_url: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiConfigs {
    pub binance: ExchangeConfig,
    pub coinbase: ExchangeConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralSettings {
    pub fetch_interval_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub apis: ApiConfigs,
    pub general: GeneralSettings,
    // We could add other general settings here later, e.g., logging level, aggregation strategy, etc.
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        let builder = Config::builder()
            // Start with `./config/default.toml`
            .add_source(File::with_name("config/default").required(true))
            // Add in `./config/local.toml` to override defaults
            .add_source(File::with_name("config/local").required(false));

        builder.build()?.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    // Helper to create a temporary config file for testing
    fn create_temp_config_file(dir: &str, name: &str, content: &str) -> Result<()> {
        fs::create_dir_all(dir)?;
        let path = format!("{}/{}.toml", dir, name);
        let mut file = fs::File::create(path)?;
        writeln!(file, "{}", content)?;
        Ok(())
    }

    #[test]
    fn test_load_config_defaults_only() -> Result<()> {
        // Create a dummy default.toml
        let config_dir = "./test_config_load_defaults";
        create_temp_config_file(
            config_dir,
            "default",
            r#"
[apis.binance]
base_url = "https://api.binance.com/api/v3"
symbols = ["BTCUSDT", "ETHUSDT"]

[apis.coinbase]
base_url = "https://api.exchange.coinbase.com"
symbols = ["BTC-USD", "ETH-USD"]

[general]
fetch_interval_seconds = 5
        "#,
        )?;

        let s = Config::builder()
            .add_source(File::with_name(&format!("{}/default", config_dir)).required(true))
            .build()?;
        let settings: Settings = s.try_deserialize()?;

        assert_eq!(
            settings.apis.binance.base_url,
            "https://api.binance.com/api/v3"
        );
        assert_eq!(settings.apis.binance.symbols, vec!["BTCUSDT", "ETHUSDT"]);
        assert_eq!(
            settings.apis.coinbase.base_url,
            "https://api.exchange.coinbase.com"
        );
        assert_eq!(settings.apis.coinbase.symbols, vec!["BTC-USD", "ETH-USD"]);
        assert_eq!(settings.general.fetch_interval_seconds, 5);

        // Clean up
        fs::remove_dir_all(config_dir)?;
        Ok(())
    }

    #[test]
    fn test_load_config_with_local_override() -> Result<()> {
        let config_dir = "./test_config_load_local";
        // Default config
        create_temp_config_file(
            config_dir,
            "default",
            r#"
[apis.binance]
base_url = "https://api.binance.com/api/v3"
symbols = ["BTCUSDT", "ETHUSDT"]

[apis.coinbase]
base_url = "https://api.exchange.coinbase.com"
symbols = ["BTC-USD", "ETH-USD"]

[general]
fetch_interval_seconds = 10 # Default interval
        "#,
        )?;

        // Local override for binance url and one symbol, and fetch interval
        create_temp_config_file(
            config_dir,
            "local",
            r#"
[apis.binance]
base_url = "http://localhost:8080/binance"
symbols = ["DOGEUSDT"]

[general]
fetch_interval_seconds = 3 # Override interval
        "#,
        )?;

        let s = Config::builder()
            .add_source(File::with_name(&format!("{}/default", config_dir)).required(true))
            .add_source(File::with_name(&format!("{}/local", config_dir)).required(false))
            .build()?;
        let settings: Settings = s.try_deserialize()?;

        // Binance should be overridden
        assert_eq!(
            settings.apis.binance.base_url,
            "http://localhost:8080/binance"
        );
        assert_eq!(settings.apis.binance.symbols, vec!["DOGEUSDT"]);
        // Coinbase should remain default
        assert_eq!(
            settings.apis.coinbase.base_url,
            "https://api.exchange.coinbase.com"
        );
        assert_eq!(settings.apis.coinbase.symbols, vec!["BTC-USD", "ETH-USD"]);
        assert_eq!(settings.general.fetch_interval_seconds, 3);

        fs::remove_dir_all(config_dir)?;
        Ok(())
    }
}
