use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// In-memory config store for session configuration.
#[derive(Clone)]
pub struct ConfigStore {
    data: Arc<RwLock<HashMap<String, Value>>>,
}

impl ConfigStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// ConfigTool for get/set/list session settings.
pub struct ConfigTool {
    store: ConfigStore,
}

impl ConfigTool {
    pub fn new(store: ConfigStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ConfigTool {
    fn name(&self) -> &str {
        "Config"
    }

    fn description(&self) -> &str {
        "Get or set configuration values. Supports session-scoped settings."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "action".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["get", "set", "list"],
                        "description": "Operation to perform"
                    }),
                ),
                (
                    "key".to_string(),
                    json!({ "type": "string", "description": "Config key" }),
                ),
                (
                    "value".to_string(),
                    json!({ "description": "Config value (for set)" }),
                ),
            ]),
            required: vec!["action".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_read_only(&self, input: &Value) -> bool {
        input
            .get("action")
            .and_then(|a| a.as_str())
            .map(|a| a == "get" || a == "list")
            .unwrap_or(false)
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'action'".to_string()))?;

        match action {
            "get" => {
                let key = input
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidInput("key required for get".to_string()))?;

                let data = self.store.data.read().await;
                match data.get(key) {
                    Some(value) => Ok(ToolResult::text(serde_json::to_string(value).unwrap_or_default())),
                    None => Ok(ToolResult::text(format!("Config key \"{}\" not found", key))),
                }
            }
            "set" => {
                let key = input
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidInput("key required for set".to_string()))?;

                let value = input.get("value").cloned().unwrap_or(Value::Null);

                let mut data = self.store.data.write().await;
                data.insert(key.to_string(), value.clone());

                Ok(ToolResult::text(format!(
                    "Config set: {} = {}",
                    key,
                    serde_json::to_string(&value).unwrap_or_default()
                )))
            }
            "list" => {
                let data = self.store.data.read().await;

                if data.is_empty() {
                    return Ok(ToolResult::text("No config values set."));
                }

                let mut entries: Vec<(&String, &Value)> = data.iter().collect();
                entries.sort_by_key(|(k, _)| (*k).clone());

                let lines: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| {
                        format!("{} = {}", k, serde_json::to_string(v).unwrap_or_default())
                    })
                    .collect();

                Ok(ToolResult::text(lines.join("\n")))
            }
            _ => Ok(ToolResult::error(format!("Unknown action: {}", action))),
        }
    }
}
