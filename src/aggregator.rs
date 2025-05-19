/// Aggregates a list of optional price points into a single optional average price.
///
/// - Filters out `None` values (representing failures from a source).
/// - If no valid prices remain, returns `None`.
/// - Otherwise, calculates the arithmetic mean of the valid prices.
pub fn aggregate_prices(price_options: &[Option<f64>]) -> Option<f64> {
    let valid_prices: Vec<f64> = price_options
        .iter()
        .filter_map(|&opt_price| opt_price)
        .collect();

    if valid_prices.is_empty() {
        None
    } else {
        let sum: f64 = valid_prices.iter().sum();
        Some(sum / valid_prices.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELTA: f64 = 1e-9; // For floating point comparisons

    #[test]
    fn test_aggregate_two_valid_prices() {
        let prices = [Some(100.0), Some(102.0)];
        let aggregated = aggregate_prices(&prices).unwrap();
        assert!((aggregated - 101.0).abs() < DELTA);
    }

    #[test]
    fn test_aggregate_one_valid_one_none() {
        let prices = [Some(100.0), None];
        let aggregated = aggregate_prices(&prices).unwrap();
        assert!((aggregated - 100.0).abs() < DELTA);
    }

    #[test]
    fn test_aggregate_one_none_one_valid() {
        let prices = [None, Some(102.0)];
        let aggregated = aggregate_prices(&prices).unwrap();
        assert!((aggregated - 102.0).abs() < DELTA);
    }

    #[test]
    fn test_aggregate_two_none_prices() {
        let prices = [None, None];
        assert_eq!(aggregate_prices(&prices), None);
    }

    #[test]
    fn test_aggregate_empty_input() {
        let prices: [Option<f64>; 0] = [];
        assert_eq!(aggregate_prices(&prices), None);
    }

    #[test]
    fn test_aggregate_multiple_valid_prices() {
        let prices = [Some(10.0), Some(20.0), Some(30.0)];
        let aggregated = aggregate_prices(&prices).unwrap();
        assert!((aggregated - 20.0).abs() < DELTA);
    }

    #[test]
    fn test_aggregate_single_valid_price() {
        let prices = [Some(123.45)];
        let aggregated = aggregate_prices(&prices).unwrap();
        assert!((aggregated - 123.45).abs() < DELTA);
    }

    #[test]
    fn test_with_real_world_like_values() {
        let prices = [Some(60100.50), Some(60102.30), None, Some(60098.10)];
        let expected_avg = (60100.50 + 60102.30 + 60098.10) / 3.0;
        let aggregated = aggregate_prices(&prices).unwrap();
        assert!((aggregated - expected_avg).abs() < DELTA);
    }
}
