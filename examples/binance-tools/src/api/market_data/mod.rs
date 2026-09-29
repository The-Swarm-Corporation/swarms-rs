#![allow(non_snake_case)]

pub mod agg_trades;
pub mod avg_price;
pub mod depth;
pub mod historical_trades;
pub mod klines;
pub mod ticker_24hr;
pub mod ticker_book_ticker;
pub mod ticker_price;
pub mod ticker_rolling_window_price;
pub mod ticker_trading_day;
pub mod trades;
pub mod ui_klines;

pub use agg_trades::agg_trades;
pub use avg_price::avg_price;
pub use depth::depth;
pub use historical_trades::historical_trades;
pub use klines::klines;
pub use ticker_24hr::ticker_24hr;
pub use ticker_book_ticker::ticker_book_ticker;
pub use ticker_price::ticker_price;
pub use ticker_rolling_window_price::ticker_rolling_window_price;
pub use ticker_trading_day::ticker_trading_day;
pub use trades::trades;
pub use ui_klines::ui_klines;

/// Binance expects `symbols` as a JSON array string (`symbols=["BTCUSDT","BNBUSDT"]`), and
/// URL-encoding a `Vec` directly fails, so serialize it through JSON.
pub(crate) fn symbols_as_json<S: serde::Serializer>(
    symbols: &Option<Vec<String>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match symbols {
        Some(symbols) => serializer
            .serialize_str(&serde_json::to_string(symbols).map_err(serde::ser::Error::custom)?),
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_sent_as_a_json_array_string() {
        let params = ticker_price::TickerPriceRequest {
            symbol: None,
            symbols: Some(vec!["BTCUSDT".to_string(), "BNBUSDT".to_string()]),
        };
        let request = reqwest::Client::new()
            .get("https://api.binance.com/api/v3/ticker/price")
            .query(&params)
            .build()
            .unwrap();
        assert_eq!(
            request.url().query(),
            Some("symbols=%5B%22BTCUSDT%22%2C%22BNBUSDT%22%5D")
        );

        let single = ticker_price::TickerPriceRequest {
            symbol: Some("BTCUSDT".to_string()),
            symbols: None,
        };
        let request = reqwest::Client::new()
            .get("https://api.binance.com/api/v3/ticker/price")
            .query(&single)
            .build()
            .unwrap();
        assert_eq!(request.url().query(), Some("symbol=BTCUSDT"));
    }

    #[test]
    fn trades_endpoints_decode_arrays() {
        // Shape of GET /api/v3/trades and /api/v3/historicalTrades responses.
        let body = r#"[{"id":28457,"price":"4.00000100","qty":"12.00000000","quoteQty":"48.000012",
                        "time":1499865549590,"isBuyerMaker":true,"isBestMatch":true}]"#;
        let trades: Vec<trades::TradesResponse> = serde_json::from_str(body).unwrap();
        assert_eq!(trades[0].id, 28457);
        let older: Vec<historical_trades::HistoricalTradesResponse> =
            serde_json::from_str(body).unwrap();
        assert_eq!(older.len(), 1);
    }
}
