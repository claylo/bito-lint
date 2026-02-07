//! MCP (Model Context Protocol) server implementation.
//!
//! This module exposes project functionality over the MCP protocol, making it
//! available to AI assistants (Claude Code, Cursor, etc.) via stdio transport.
//!
//! # Architecture
//!
//! The MCP server is a presentation layer — it wraps the same core library that
//! the CLI commands use. Each `#[tool]` method should delegate to core library
//! functions rather than implementing business logic directly.
//!
//! # Adding Tools
//!
//! 1. Define a parameter struct with `Deserialize` + `JsonSchema`
//! 2. Add a `#[tool(description = "...")]` method to the `#[tool_router]` impl
//! 3. Call core library functions, convert errors to `McpError`
//! 4. Return `CallToolResult::success(vec![Content::text(...)])`

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo,
};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use rmcp::schemars;


/// Parameters for the `get_info` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetInfoParams {
    /// Output format: "text" or "json"
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "text".to_string()
}

/// MCP server exposing project functionality to AI assistants.
///
/// Each `#[tool]` method in the `#[tool_router]` impl block is automatically
/// registered and callable via the MCP protocol.
#[derive(Clone)]
pub struct ProjectServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl Default for ProjectServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl ProjectServer {
    /// Create a new MCP server instance.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Get project information.
    #[tool(description = "Get project name, version, and description")]
    #[tracing::instrument(skip(self), fields(otel.kind = "server"))]
    fn get_info(
        &self,
        #[allow(unused_variables)]
        Parameters(params): Parameters<GetInfoParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(tool = "get_info", format = %params.format, "executing MCP tool");

        let info = serde_json::json!({
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "description": env!("CARGO_PKG_DESCRIPTION"),
        });

        let text = if params.format == "json" {
            serde_json::to_string_pretty(&info)
                .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?
        } else {
            format!(
                "{} v{}\n{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_DESCRIPTION"),
            )
        };

        tracing::info!(tool = "get_info", "MCP tool completed");
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

}

#[tool_handler]
impl ServerHandler for ProjectServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            instructions: Some(format!(
                "{} MCP server. Use tools to interact with project functionality.",
                env!("CARGO_PKG_NAME"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::RawContent;

    #[test]
    fn server_info_has_correct_name() {
        let server = ProjectServer::new();
        let info = ServerHandler::get_info(&server);

        assert_eq!(info.server_info.name, env!("CARGO_PKG_NAME"));
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn server_has_tools_capability() {
        let server = ProjectServer::new();
        let info = ServerHandler::get_info(&server);

        assert!(info.capabilities.tools.is_some());
    }

    #[test]
    fn server_has_instructions() {
        let server = ProjectServer::new();
        let info = ServerHandler::get_info(&server);

        let instructions = info.instructions.expect("server should have instructions");
        assert!(instructions.contains(env!("CARGO_PKG_NAME")));
    }

    /// Extract text from the first content item in a CallToolResult.
    fn extract_text(result: &CallToolResult) -> Option<&str> {
        result.content.first().and_then(|c| match &c.raw {
            RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
    }

    #[test]
    fn get_info_tool_returns_text_by_default() {
        let server = ProjectServer::new();
        let params = Parameters(GetInfoParams {
            format: "text".to_string(),
        });

        let result = server.get_info(params).expect("get_info should succeed");

        assert!(!result.is_error.unwrap_or(false));
        assert!(!result.content.is_empty());

        let text = extract_text(&result).expect("should have text content");
        assert!(text.contains(env!("CARGO_PKG_NAME")));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn get_info_tool_returns_json_when_requested() {
        let server = ProjectServer::new();
        let params = Parameters(GetInfoParams {
            format: "json".to_string(),
        });

        let result = server.get_info(params).expect("get_info should succeed");

        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");

        // Verify it's valid JSON
        let json: serde_json::Value =
            serde_json::from_str(text).expect("output should be valid JSON");

        assert_eq!(json["name"], env!("CARGO_PKG_NAME"));
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

}
