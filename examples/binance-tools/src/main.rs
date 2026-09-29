use std::env;

use mcp_tools::BinanceMCPTools;
use rmcp::{
    ServiceExt,
    transport::{SseServer, stdio},
};
use tracing_subscriber::EnvFilter;

// examples/binance-mcp/src/main.rs
mod api;
mod auth;
mod mcp_tools;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Binance MCP Server...");

    let (ctrlc_tx, ctrlc_rx) = tokio::sync::oneshot::channel::<()>();
    let (sse_cancel_tx, sse_cancel_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        // Local only by default (as documented); set BINANCE_MCP_SSE_ADDR to expose it.
        let sse_addr = env::var("BINANCE_MCP_SSE_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8000".to_owned())
            .parse()
            .expect("Invalid SSE address");
        tracing::info!("Starting SSE server at {}", sse_addr);
        // If the port is taken (e.g. another instance), keep serving over stdio.
        let ct = match SseServer::serve(sse_addr).await {
            Ok(server) => server.with_service(BinanceMCPTools::new),
            Err(e) => {
                tracing::error!("Failed to start SSE server at {sse_addr}: {e}");
                return;
            }
        };

        tokio::select! {
            _ = ctrlc_rx => {
                ct.cancel();
                let _ = sse_cancel_tx.send(());
            }
        }
    });

    tracing::info!("Starting STDIO server...");
    let service = BinanceMCPTools::new().serve(stdio()).await?;

    tokio::signal::ctrl_c().await?;
    tracing::info!("Stopping Binance MCP Server...");

    // The SSE task may already have exited (e.g. its port was taken).
    let _ = ctrlc_tx.send(());
    service.cancel().await?;
    let _ = sse_cancel_rx.await;

    tracing::info!("Binance MCP Server stopped.");
    Ok(())
}
