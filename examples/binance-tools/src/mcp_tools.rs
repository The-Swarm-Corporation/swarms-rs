use reqwest::Client;
use rmcp::{
    ErrorData as MCPError, ServerHandler,
    model::{CallToolResult, Content, ServerInfo},
    tool,
};

use crate::api::market_data::{
    self, agg_trades, avg_price, depth, historical_trades, klines, ticker_24hr, ticker_book_ticker,
    ticker_price, ticker_rolling_window_price, ticker_trading_day, trades, ui_klines,
};

macro_rules! call_api_tool {
    ($self:expr, $api_function:path, $params:expr) => {
        match $api_function(&$self.client, &$self.base_url, $params).await {
            Ok(res) => CallToolResult::success(vec![Content::json(res)?]),
            Err(err) => CallToolResult::error(vec![Content::text(err.to_string())]),
        }
    };
}

#[derive(Clone)]
pub struct BinanceMCPTools {
    pub base_url: String,
    pub client: Client,
}

impl BinanceMCPTools {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.binance.com".to_owned(),
            client: Client::new(),
        }
    }

    #[tool(
        description = "Get compressed, aggregate trades. Trades that fill at the time, from the same taker order, with the same price will have the quantity aggregated."
    )]
    async fn agg_trades(
        &self,
        params: agg_trades::AggTradesRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::agg_trades, params);
        Ok(result)
    }

    #[tool(description = "Get current average price for a symbol.")]
    async fn avg_price(
        &self,
        params: avg_price::AvgPriceRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::avg_price, params);
        Ok(result)
    }

    #[tool(description = "Get depth information.")]
    async fn depth(
        &self,
        params: depth::DepthRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::depth, params);
        Ok(result)
    }

    #[tool(description = "Get older trades.")]
    async fn historical_trades(
        &self,
        params: historical_trades::HistoricalTradesRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::historical_trades, params);
        Ok(result)
    }

    #[tool(description = "
        Kline/candlestick bars for a symbol. Klines are uniquely identified by their open time.
        
        Response array:
            0: Kline open time
            1: Open price
            2: High price
            3: Low price
            4: Close price
            5: Volume
            6: Kline close time
            7: Quote asset volume
            8: Number of trades
            9: Taker buy base asset volume
            10: Taker buy quote asset volume
            11: Unused field. Ignore.
        ")]
    async fn klines(
        &self,
        params: klines::KlinesRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::klines, params);
        Ok(result)
    }

    #[tool(
        description = "24 hour rolling window price change statistics. Careful when accessing this with no symbol."
    )]
    async fn ticker_24hr(
        &self,
        params: ticker_24hr::Ticker24HrRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::ticker_24hr, params);
        Ok(result)
    }

    #[tool(description = "Best price/qty on the order book for all symbols.")]
    async fn ticker_book_ticker(
        &self,
        params: ticker_book_ticker::TickerBookTickerRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::ticker_book_ticker, params);
        Ok(result)
    }

    #[tool(description = "Latest price for all symbols or for a symbol.")]
    async fn ticker_price(
        &self,
        params: ticker_price::TickerPriceRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::ticker_price, params);
        Ok(result)
    }

    #[tool(description = "Latest price for a symbol with 24 hour rolling window.")]
    async fn ticker_rolling_window_price(
        &self,
        params: ticker_rolling_window_price::TickerRollingWindowPriceRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::ticker_rolling_window_price, params);
        Ok(result)
    }

    #[tool(description = "Price change statistics for a trading day.")]
    async fn ticker_trading_day(
        &self,
        params: ticker_trading_day::TickerTradingDayRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::ticker_trading_day, params);
        Ok(result)
    }

    #[tool(description = "Recent trades list.")]
    async fn trades(
        &self,
        params: trades::TradesRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::trades, params);
        Ok(result)
    }

    #[tool(description = "
        The request is similar to klines having the same parameters and response. uiKlines return modified kline data, optimized for presentation of candlestick charts.
                
        Response array:
            0: Kline open time
            1: Open price
            2: High price
            3: Low price
            4: Close price
            5: Volume
            6: Kline close time
            7: Quote asset volume
            8: Number of trades
            9: Taker buy base asset volume
            10: Taker buy quote asset volume
            11: Unused field. Ignore.
        ")]
    async fn ui_klines(
        &self,
        params: ui_klines::UIKlinesRequest,
    ) -> Result<CallToolResult, MCPError> {
        let result = call_api_tool!(self, market_data::ui_klines, params);
        Ok(result)
    }
}

impl ServerHandler for BinanceMCPTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Binance API".to_owned()),
            ..Default::default()
        }
    }
}
