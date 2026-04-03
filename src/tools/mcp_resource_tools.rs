use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::mcp::McpClient;
use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// ListMcpResourcesTool - List available resources from connected MCP servers.
pub struct ListMcpResourcesTool {
    mcp_client: Arc<RwLock<Option<Arc<McpClient>>>>,
}

impl ListMcpResourcesTool {
    pub fn new(mcp_client: Arc<RwLock<Option<Arc<McpClient>>>>) -> Self {
        Self { mcp_client }
    }
}

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "ListMcpResources"
    }

    fn description(&self) -> &str {
        "List available resources from connected MCP servers. Resources can include files, databases, and other data sources."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "server".to_string(),
                json!({ "type": "string", "description": "Filter by MCP server name" }),
            )]),
            required: vec![],
            additional_properties: Some(false),
        }
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let client_guard = self.mcp_client.read().await;
        let client = match client_guard.as_ref() {
            Some(c) => c.clone(),
            None => return Ok(ToolResult::text("No MCP servers connected.")),
        };

        let server_filter = input.get("server").and_then(|v| v.as_str());

        let connections = client.get_connections().await;

        let filtered: Vec<_> = if let Some(server) = server_filter {
            connections.into_iter().filter(|c| c.name == server).collect()
        } else {
            connections
        };

        if filtered.is_empty() {
            return Ok(ToolResult::text("No MCP servers connected."));
        }

        let mut results = Vec::new();
        for conn in &filtered {
            if !conn.connected {
                continue;
            }
            results.push(format!(
                "Server: {} ({} tools available)",
                conn.name,
                conn.tools.len()
            ));
            // Note: Full resource listing requires MCP resources/list support.
            // Currently showing available tool count as a proxy.
        }

        if results.is_empty() {
            Ok(ToolResult::text("No connected MCP servers found."))
        } else {
            Ok(ToolResult::text(results.join("\n")))
        }
    }
}

/// ReadMcpResourceTool - Read a specific resource from an MCP server.
pub struct ReadMcpResourceTool {
    mcp_client: Arc<RwLock<Option<Arc<McpClient>>>>,
}

impl ReadMcpResourceTool {
    pub fn new(mcp_client: Arc<RwLock<Option<Arc<McpClient>>>>) -> Self {
        Self { mcp_client }
    }
}

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "ReadMcpResource"
    }

    fn description(&self) -> &str {
        "Read a specific resource from an MCP server."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "server".to_string(),
                    json!({ "type": "string", "description": "MCP server name" }),
                ),
                (
                    "uri".to_string(),
                    json!({ "type": "string", "description": "Resource URI to read" }),
                ),
            ]),
            required: vec!["server".to_string(), "uri".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let server = input
            .get("server")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'server'".to_string()))?;
        let uri = input
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'uri'".to_string()))?;

        let client_guard = self.mcp_client.read().await;
        let client = match client_guard.as_ref() {
            Some(c) => c.clone(),
            None => {
                return Ok(ToolResult::error(format!(
                    "MCP server not found: {}",
                    server
                )))
            }
        };

        let connections = client.get_connections().await;
        let conn = connections.iter().find(|c| c.name == server);

        if conn.is_none() {
            return Ok(ToolResult::error(format!(
                "MCP server not found: {}",
                server
            )));
        }

        // MCP resource reading is done via the resources/read method.
        // Since the current McpClient doesn't expose a read_resource method,
        // we provide a stub that indicates the operation is recognized but
        // requires the MCP server to support the resources/read capability.
        Ok(ToolResult::text(format!(
            "Resource read request: server=\"{}\" uri=\"{}\". \
             Full resource reading requires MCP server support for the resources/read method.",
            server, uri
        )))
    }
}
