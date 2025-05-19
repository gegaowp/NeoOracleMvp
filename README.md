# Neo Oracle MVP

## Overview

This project is a Minimum Viable Product (MVP) for a cryptocurrency price oracle. It sources price data for primary asset pairs (BTC/USD, ETH/USD) from multiple exchanges, aggregates them, and publishes these prices to the Sui Testnet blockchain.

This MVP is built in Rust.

## Current Features

*   **Data Sourcing**: Fetches ticker prices for BTC and ETH from:
    *   Binance (BTCUSDT, ETHUSDT)
    *   Coinbase (BTC-USD, ETH-USD)
*   **Price Aggregation**: Calculates a simple average of the prices obtained from the different sources for each asset pair.
*   **Sui On-Chain Publisher**: 
    *   Manages on-chain PriceObject instances for BTC/USD and ETH/USD via the `sui_publisher.rs` module.
    *   Creates PriceObjects if they don't exist (tracks known objects in `known_price_objects.json`).
    *   Updates existing PriceObjects with the latest aggregated prices and timestamps.
    *   Interacts with a specified Move package on the Sui Testnet.
*   **Configuration**: API endpoints, symbols, Sui package details, and general settings (like fetch interval) are configurable via `config/default.toml`. Local overrides can be placed in `config/local.toml`.
*   **Continuous Operation**: Runs in a loop, fetching, aggregating, and publishing prices at a configurable interval (default: 5 seconds).
*   **Logging**: Outputs informational logs about its operations using the `log` crate, configurable via `RUST_LOG`.

## How to Run

### Prerequisites

*   Rust programming language and Cargo package manager installed (latest stable version recommended).
*   Internet connection for fetching live price data and interacting with the Sui Testnet.

### Steps

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/gegaowp/NeoOracleMvp.git
    cd NeoOracleMvp/neo_oracle_mvp 
    ```

2.  **Run the application:**
    ```bash
    RUST_LOG=info cargo run
    ```
    This will start the oracle. You should see log output in your console showing fetched, aggregated, and published prices, updating at the configured interval.

3.  **To stop the application:**
    Press `Ctrl+C` in the terminal where it's running.

## Configuration

The application uses a TOML configuration file located at `neo_oracle_mvp/config/default.toml`.

Key configurable items:
*   Exchange API base URLs and symbols.
*   Sui network details (RPC URL, package ID) are currently constants in `sui_publisher.rs` but could be moved to config.
*   `general.fetch_interval_seconds` in `config/default.toml`.

To override default settings locally, create `neo_oracle_mvp/config/local.toml`.

## Key Modules

*   `main.rs`: Main application loop, orchestrates fetching, aggregation, and publishing.
*   `binance_client.rs`, `coinbase_client.rs`: Clients for fetching data from exchanges.
*   `aggregator.rs`: Logic for price aggregation.
*   `sui_publisher.rs`: Handles all interactions with the Sui blockchain (creating/updating price objects).
*   `config.rs`: Manages application configuration.

## Next Steps (Future Phases)

*   Move Sui-specific constants (Package ID, RPC URL) from code to the configuration file.
*   More sophisticated aggregation strategies (e.g., weighted average, outlier detection).
*   Expanded data source support (more exchanges, different asset types).
*   Enhanced error handling, resilience, and monitoring.
*   Comprehensive on-chain contract upgrades and management features. 
