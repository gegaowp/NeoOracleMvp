# Neo Oracle MVP

## Overview

This project is a Minimum Viable Product (MVP) for a cryptocurrency price oracle. It currently focuses on sourcing price data for primary asset pairs (BTC/USD, ETH/USD) from multiple exchanges, aggregating them, and displaying the results. The eventual goal is to deliver this price feed to the Sui blockchain.

This MVP is built in Rust.

## Current Features (Phase 1)

*   **Data Sourcing**: Fetches ticker prices for BTC and ETH from:
    *   Binance (BTCUSDT, ETHUSDT)
    *   Coinbase (BTC-USD, ETH-USD)
*   **Price Aggregation**: Calculates a simple average of the prices obtained from the different sources for each asset pair.
*   **Configuration**: API endpoints and symbols are configurable via `config/default.toml`. Local overrides can be placed in `config/local.toml` (which is gitignored).
*   **Continuous Operation**: Runs in a loop, fetching and aggregating prices every 5 seconds.
*   **Logging**: Outputs informational logs about its operations, including fetched and aggregated prices.

## How to Run

### Prerequisites

*   Rust programming language and Cargo package manager installed (latest stable version recommended).
*   Internet connection for fetching live price data.

### Steps

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/gegaowp/NeoOracleMvp.git
    cd NeoOracleMvp
    ```

2.  **Run the application:**
    ```bash
    RUST_LOG=info cargo run
    ```
    This will start the oracle. You should see log output in your console showing the fetched and aggregated prices, updating every 5 seconds.

3.  **To stop the application:**
    Press `Ctrl+C` in the terminal where it's running.

## Configuration

The application uses a TOML configuration file located at `config/default.toml`.

Key configurable items:
*   `apis.binance.base_url`: Base URL for Binance API.
*   `apis.binance.symbols`: List of symbols to fetch from Binance (e.g., `["BTCUSDT", "ETHUSDT"]`).
*   `apis.coinbase.base_url`: Base URL for Coinbase API.
*   `apis.coinbase.symbols`: List of symbols to fetch from Coinbase (e.g., `["BTC-USD", "ETH-USD"]`).

To override default settings locally (e.g., for different API keys if they were used, or different symbols), you can create a `config/local.toml` file. This file is not tracked by Git.

## Next Steps (Future Phases)

*   Integration with the Sui blockchain to publish the aggregated price data.
*   More sophisticated aggregation strategies.
*   Expanded data source support.
*   Enhanced error handling and resilience. 
