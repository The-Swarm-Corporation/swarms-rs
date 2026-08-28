# Binance tools

- [x] Market Data endpoints
- [ ] Trading endpoints
- [ ] Account endpoints

## Environment Variables

- `BINANCE_MCP_HTTP_ADDR` - Binance MCP Streamable HTTP address, default: `0.0.0.0:8000`

## Usage

```shell
cargo run --package binance-tools --release
```

Or (debug mode)
```shell
cargo run --package binance-tools
```

Both STDIO and Streamable HTTP MCP server enabled.

Streamable HTTP MCP server: `http://BINANCE_MCP_HTTP_ADDR/mcp`