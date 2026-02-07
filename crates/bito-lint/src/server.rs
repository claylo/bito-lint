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
use rmcp::model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo};
use rmcp::schemars;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

use bito_lint_core::{analysis, completeness, grammar, readability, tokens};

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

/// Parameters for the `count_tokens` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CountTokensParams {
    /// The text to count tokens in.
    pub text: String,
    /// Optional maximum token budget.
    pub budget: Option<usize>,
}

/// Parameters for the `check_readability` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckReadabilityParams {
    /// The text to analyze.
    pub text: String,
    /// Maximum acceptable Flesch-Kincaid grade level.
    pub max_grade: Option<f64>,
    /// Whether to strip markdown formatting before analysis.
    #[serde(default)]
    pub strip_markdown: bool,
}

/// Parameters for the `check_completeness` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckCompletenessParams {
    /// The markdown document text.
    pub text: String,
    /// Template to validate against: "adr", "handoff", or "design-doc".
    pub template: String,
}

/// Parameters for the `check_grammar` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckGrammarParams {
    /// The text to analyze.
    pub text: String,
    /// Whether to strip markdown formatting before analysis.
    #[serde(default)]
    pub strip_markdown: bool,
    /// Maximum acceptable passive voice percentage (0-100).
    pub passive_max: Option<f64>,
}

/// Parameters for the `analyze_writing` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeWritingParams {
    /// The text to analyze.
    pub text: String,
    /// Whether to strip markdown formatting before analysis.
    #[serde(default)]
    pub strip_markdown: bool,
    /// Checks to run (comma-separated). Omit for all checks.
    pub checks: Option<Vec<String>>,
    /// Maximum acceptable readability grade level.
    pub max_grade: Option<f64>,
    /// Maximum acceptable passive voice percentage.
    pub passive_max: Option<f64>,
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
        #[allow(unused_variables)] Parameters(params): Parameters<GetInfoParams>,
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

    /// Count tokens in text using cl100k_base tokenizer (approximates Claude usage).
    #[tool(description = "Count tokens in text. Returns token count and optional budget check.")]
    #[tracing::instrument(skip(self, params), fields(otel.kind = "server"))]
    fn count_tokens(
        &self,
        #[allow(unused_variables)] Parameters(params): Parameters<CountTokensParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(tool = "count_tokens", budget = ?params.budget, "executing MCP tool");

        let report = tokens::count_tokens(&params.text, params.budget)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;

        tracing::info!(
            tool = "count_tokens",
            count = report.count,
            "MCP tool completed"
        );
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Score readability using Flesch-Kincaid Grade Level.
    #[tool(
        description = "Check readability of text. Returns Flesch-Kincaid grade level and statistics."
    )]
    #[tracing::instrument(skip(self, params), fields(otel.kind = "server"))]
    fn check_readability(
        &self,
        #[allow(unused_variables)] Parameters(params): Parameters<CheckReadabilityParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(
            tool = "check_readability",
            strip_md = params.strip_markdown,
            "executing MCP tool"
        );

        let report =
            readability::check_readability(&params.text, params.strip_markdown, params.max_grade)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;

        tracing::info!(
            tool = "check_readability",
            grade = report.grade,
            "MCP tool completed"
        );
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Check document completeness against a template.
    #[tool(
        description = "Validate that a markdown document has all required sections for a template (adr, handoff, design-doc)."
    )]
    #[tracing::instrument(skip(self, params), fields(otel.kind = "server", template = %params.template))]
    fn check_completeness(
        &self,
        #[allow(unused_variables)] Parameters(params): Parameters<CheckCompletenessParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(tool = "check_completeness", template = %params.template, "executing MCP tool");

        let report = completeness::check_completeness(&params.text, &params.template, None)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;

        tracing::info!(
            tool = "check_completeness",
            pass = report.pass,
            "MCP tool completed"
        );
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Run comprehensive writing analysis.
    #[tool(
        description = "Analyze writing quality across 18 dimensions: readability, grammar, style, pacing, transitions, overused words, cliches, jargon, and more."
    )]
    #[tracing::instrument(skip(self, params), fields(otel.kind = "server"))]
    fn analyze_writing(
        &self,
        #[allow(unused_variables)] Parameters(params): Parameters<AnalyzeWritingParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(
            tool = "analyze_writing",
            strip_md = params.strip_markdown,
            "executing MCP tool"
        );

        let checks_ref = params.checks.as_deref();
        let report = analysis::run_full_analysis(
            &params.text,
            params.strip_markdown,
            checks_ref,
            params.max_grade,
            params.passive_max,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;

        tracing::info!(tool = "analyze_writing", "MCP tool completed");
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Check grammar and passive voice in text.
    #[tool(
        description = "Check grammar issues and passive voice usage. Returns grammar issues with severity and passive voice statistics."
    )]
    #[tracing::instrument(skip(self, params), fields(otel.kind = "server"))]
    fn check_grammar(
        &self,
        #[allow(unused_variables)] Parameters(params): Parameters<CheckGrammarParams>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(
            tool = "check_grammar",
            strip_md = params.strip_markdown,
            "executing MCP tool"
        );

        let report =
            grammar::check_grammar_full(&params.text, params.strip_markdown, params.passive_max)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;

        tracing::info!(
            tool = "check_grammar",
            passive_count = report.passive_count,
            issue_count = report.issues.len(),
            "MCP tool completed"
        );
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for ProjectServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
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

    /// Extract text from the first content item in a `CallToolResult`.
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

    #[test]
    fn count_tokens_tool_works() {
        let server = ProjectServer::new();
        let params = Parameters(CountTokensParams {
            text: "Hello, world!".to_string(),
            budget: Some(100),
        });

        let result = server
            .count_tokens(params)
            .expect("count_tokens should succeed");
        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");
        let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        assert!(json["count"].as_u64().unwrap() > 0);
        assert!(!json["over_budget"].as_bool().unwrap());
    }

    #[test]
    fn check_readability_tool_works() {
        let server = ProjectServer::new();
        let params = Parameters(CheckReadabilityParams {
            text: "The cat sat on the mat. The dog ran fast.".to_string(),
            max_grade: None,
            strip_markdown: false,
        });

        let result = server
            .check_readability(params)
            .expect("check_readability should succeed");
        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");
        let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        assert!(json["grade"].as_f64().is_some());
        assert!(json["words"].as_u64().unwrap() > 0);
    }

    #[test]
    fn check_completeness_tool_works() {
        let server = ProjectServer::new();
        let doc = "## Where things stand\n\nDone.\n\n## Decisions made\n\nX.\n\n## What's next\n\nY.\n\n## Landmines\n\nZ.";
        let params = Parameters(CheckCompletenessParams {
            text: doc.to_string(),
            template: "handoff".to_string(),
        });

        let result = server
            .check_completeness(params)
            .expect("check_completeness should succeed");
        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");
        let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        assert!(json["pass"].as_bool().unwrap());
    }

    #[test]
    fn analyze_writing_tool_works() {
        let server = ProjectServer::new();
        let params = Parameters(AnalyzeWritingParams {
            text: "The cat sat on the mat. The dog ran fast. However, the bird flew away."
                .to_string(),
            strip_markdown: false,
            checks: None,
            max_grade: None,
            passive_max: None,
        });

        let result = server
            .analyze_writing(params)
            .expect("analyze_writing should succeed");
        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");
        let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        assert!(json["readability"].is_object());
        assert!(json["style"].is_object());
    }

    #[test]
    fn check_grammar_tool_works() {
        let server = ProjectServer::new();
        let params = Parameters(CheckGrammarParams {
            text: "The report was written by the team. She codes every day.".to_string(),
            strip_markdown: false,
            passive_max: None,
        });

        let result = server
            .check_grammar(params)
            .expect("check_grammar should succeed");
        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");
        let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        assert!(json["sentence_count"].as_u64().unwrap() >= 2);
        assert!(json["passive_count"].as_u64().is_some());
    }

    #[test]
    fn check_completeness_tool_detects_failure() {
        let server = ProjectServer::new();
        let params = Parameters(CheckCompletenessParams {
            text: "## Where things stand\n\nDone.".to_string(),
            template: "handoff".to_string(),
        });

        let result = server
            .check_completeness(params)
            .expect("check_completeness should succeed");
        assert!(!result.is_error.unwrap_or(false));

        let text = extract_text(&result).expect("should have text content");
        let json: serde_json::Value = serde_json::from_str(text).expect("valid JSON");
        assert!(!json["pass"].as_bool().unwrap());
    }
}
