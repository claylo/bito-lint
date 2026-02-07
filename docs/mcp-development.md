# MCP Server Development Guide

## Architecture

The MCP server (`src/server.rs`) is a presentation layer for the core library — the same relationship the CLI commands have. Both are interfaces; the core library is the source of truth.

```text
┌─────────────────┐     ┌─────────────────┐
│   CLI commands   │     │   MCP server    │
│  (src/commands/) │     │  (src/server.rs)│
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────┐   ┌──────────┘
                 ▼   ▼
          ┌──────────────┐
          │  core library │
          │ (project-core)│
          └──────────────┘
```

## Adding a New Tool

1. **Define parameters** — Create a struct deriving `Deserialize` + `JsonSchema`:

   ```rust
   #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
   pub struct MyToolParams {
       /// Description shown to AI clients
       pub required_field: String,
       #[serde(default)]
       pub optional_field: Option<i32>,
   }
   ```

2. **Add the tool method** — Inside the `#[tool_router] impl ProjectServer` block:

   ```rust
   #[tool(description = "One-line description of what this tool does")]
   async fn my_tool(
       &self,
       Parameters(params): Parameters<MyToolParams>,
   ) -> Result<CallToolResult, McpError> {
       // Call core library functions here
       let result = some_core_function(params.required_field)?;
       let json = serde_json::to_string_pretty(&result)
           .map_err(|e| McpError::internal_error(format!("serialization error: {e}"), None))?;
       Ok(CallToolResult::success(vec![Content::text(json)]))
   }
   ```

3. That's it. The `#[tool_router]` macro auto-registers the method.

## Error Conversion Pattern

Map core library errors to MCP errors at the tool handler boundary:

| Core error | MCP error |
|------------|-----------|
| Item not found | `McpError::resource_not_found(msg, None)` |
| Bad input | `McpError::invalid_params(msg, None)` |
| Internal failure | `McpError::internal_error(msg, None)` |
| Serialization | `McpError::internal_error(format!("serialization error: {e}"), None)` |

## Important Constraints

- **Never write to stdout** from tool methods — stdout is the MCP transport. Use `tracing::info!()` or `eprintln!()` for diagnostics.
- **Delegate to core** — tool methods are thin wrappers. Business logic belongs in the core library crate.
- **Return JSON for structured data** — Use `serde_json::to_string_pretty()` and `Content::text()`.
- **Parameter docs matter** — `schemars` generates JSON Schema from doc comments and `#[schemars(description)]` attributes. AI clients use these to understand tool contracts.

## Testing MCP Tools

Use the MCP Inspector to test tools interactively:

```bash
npx @modelcontextprotocol/inspector cargo run -- serve
```

## Observability

Tool methods are traced via `#[tracing::instrument]`. Traces flow to any
OTLP-compatible receiver when configured.

### Quick start with otel-viewer

[otel-viewer](https://github.com/logaretm/otel-viewer) is a lightweight local
trace viewer that accepts OTLP over HTTP/JSON.

1. Run otel-viewer on port 3000
2. Set the endpoint and protocol:
   ```bash
   export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:3000/api/v1/otlp
   export OTEL_EXPORTER_OTLP_PROTOCOL=http/json
   ```
3. Start your MCP server and invoke tools — traces appear in the viewer

### Using Grafana Tempo or other gRPC collectors

The default transport is gRPC. Point at a standard OTLP gRPC endpoint:

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
```

Set `OTEL_EXPORTER_OTLP_PROTOCOL=http/json` to use HTTP/JSON instead of gRPC.

## Client Configuration

Add to Claude Code `settings.json` or `.mcp.json`:

```json
{
  "mcpServers": {
    "my-project": {
      "command": "cargo",
      "args": ["run", "--", "serve"]
    }
  }
}
```
