use std::env;

use mcp_tools::BinanceMCPTools;
use rmcp::{
    ServiceExt,
    transport::{
        streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService,
            session::local::LocalSessionManager,
        },
        stdio,
    },
};
use tokio_util::sync::CancellationToken;
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

    let (http_shutdown_tx, http_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let http_task = tokio::spawn(async move {
        let http_addr: std::net::SocketAddr = env::var("BINANCE_MCP_HTTP_ADDR")
            .unwrap_or("0.0.0.0:8000".parse().unwrap())
            .parse()
            .expect("Invalid HTTP address");
        tracing::info!("Starting Streamable HTTP server at {}", http_addr);

        let ct = CancellationToken::new();
        let service = StreamableHttpService::new(
            || Ok(BinanceMCPTools::new()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let tcp_listener = tokio::net::TcpListener::bind(http_addr)
            .await
            .expect("Failed to start HTTP server");

        axum::serve(tcp_listener, router)
            .with_graceful_shutdown(async move {
                let _ = http_shutdown_rx.await;
                ct.cancel();
            })
            .await
            .expect("HTTP server error");
    });

    tracing::info!("Starting STDIO server...");
    let service = BinanceMCPTools::new().serve(stdio()).await?;

    tokio::signal::ctrl_c().await?;
    tracing::info!("Stopping Binance MCP Server...");

    let _ = http_shutdown_tx.send(());
    let _ = http_task.await;
    service.cancel().await?;

    tracing::info!("Binance MCP Server stopped.");
    Ok(())
}
