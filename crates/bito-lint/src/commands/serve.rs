//! MCP server command implementation

use anyhow::Result;
use clap::Args;
use rmcp::ServiceExt;

use crate::server::ProjectServer;

/// Arguments for the `serve` subcommand.
#[derive(Args, Debug)]
pub struct ServeArgs {}

/// Start the MCP server on stdio.
///
/// This launches a Model Context Protocol server that communicates over
/// stdin/stdout using JSON-RPC. All logging goes to stderr to keep the
/// stdio transport clean.
#[tracing::instrument(skip_all)]
pub async fn cmd_serve(_args: ServeArgs) -> Result<()> {
    tracing::info!("starting MCP server on stdio");

    let server = ProjectServer::new();
    let service = server.serve(rmcp::transport::stdio()).await?;
    tracing::info!("MCP server ready, waiting for client");
    service.waiting().await?;
    tracing::info!("MCP server shutting down");

    Ok(())
}
