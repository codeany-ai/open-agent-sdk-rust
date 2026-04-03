use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// A todo item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u64,
    pub text: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>, // "high", "medium", "low"
}

/// In-memory todo store.
#[derive(Clone)]
pub struct TodoStore {
    items: Arc<RwLock<Vec<TodoItem>>>,
    counter: Arc<RwLock<u64>>,
}

impl TodoStore {
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(Vec::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }
}

/// TodoWriteTool - Manage a session todo/checklist.
pub struct TodoWriteTool {
    store: TodoStore,
}

impl TodoWriteTool {
    pub fn new(store: TodoStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    fn description(&self) -> &str {
        "Manage a session todo/checklist. Supports add, toggle, remove, list, and clear operations."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "action".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["add", "toggle", "remove", "list", "clear"],
                        "description": "Operation to perform"
                    }),
                ),
                (
                    "text".to_string(),
                    json!({ "type": "string", "description": "Todo item text (for add)" }),
                ),
                (
                    "id".to_string(),
                    json!({ "type": "number", "description": "Todo item ID (for toggle/remove)" }),
                ),
                (
                    "priority".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["high", "medium", "low"],
                        "description": "Priority level (for add)"
                    }),
                ),
            ]),
            required: vec!["action".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'action'".to_string()))?;

        match action {
            "add" => {
                let text = match input.get("text").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return Ok(ToolResult::error("text required")),
                };
                let priority = input
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let mut counter = self.store.counter.write().await;
                *counter += 1;
                let id = *counter;

                let item = TodoItem {
                    id,
                    text: text.to_string(),
                    done: false,
                    priority,
                };

                let mut items = self.store.items.write().await;
                items.push(item);

                Ok(ToolResult::text(format!(
                    "Todo added: #{} \"{}\"",
                    id, text
                )))
            }

            "toggle" => {
                let id = input
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

                let mut items = self.store.items.write().await;
                match items.iter_mut().find(|t| t.id == id) {
                    Some(item) => {
                        item.done = !item.done;
                        let status = if item.done { "completed" } else { "reopened" };
                        Ok(ToolResult::text(format!("Todo #{} {}", id, status)))
                    }
                    None => Ok(ToolResult::error(format!("Todo #{} not found", id))),
                }
            }

            "remove" => {
                let id = input
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

                let mut items = self.store.items.write().await;
                let len_before = items.len();
                items.retain(|t| t.id != id);

                if items.len() < len_before {
                    Ok(ToolResult::text(format!("Todo #{} removed", id)))
                } else {
                    Ok(ToolResult::error(format!("Todo #{} not found", id)))
                }
            }

            "list" => {
                let items = self.store.items.read().await;
                if items.is_empty() {
                    return Ok(ToolResult::text("No todos."));
                }

                let lines: Vec<String> = items
                    .iter()
                    .map(|t| {
                        let checkbox = if t.done { "[x]" } else { "[ ]" };
                        let priority = t
                            .priority
                            .as_ref()
                            .map(|p| format!(" ({})", p))
                            .unwrap_or_default();
                        format!("{} #{} {}{}", checkbox, t.id, t.text, priority)
                    })
                    .collect();

                Ok(ToolResult::text(lines.join("\n")))
            }

            "clear" => {
                let mut items = self.store.items.write().await;
                items.clear();
                Ok(ToolResult::text("All todos cleared."))
            }

            _ => Ok(ToolResult::error(format!("Unknown action: {}", action))),
        }
    }
}
