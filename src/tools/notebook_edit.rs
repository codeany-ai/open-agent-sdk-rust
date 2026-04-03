use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// NotebookEditTool - Edit Jupyter notebook (.ipynb) cells.
/// Supports insert, replace, and delete operations on notebook cells.
pub struct NotebookEditTool;

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "NotebookEdit"
    }

    fn description(&self) -> &str {
        "Edit Jupyter notebook (.ipynb) cells. Can insert, replace, or delete cells."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "file_path".to_string(),
                    json!({ "type": "string", "description": "Path to the .ipynb file" }),
                ),
                (
                    "command".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["insert", "replace", "delete"],
                        "description": "The edit operation to perform"
                    }),
                ),
                (
                    "cell_number".to_string(),
                    json!({
                        "type": "number",
                        "description": "Cell index (0-based) to operate on"
                    }),
                ),
                (
                    "cell_type".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["code", "markdown"],
                        "description": "Type of cell (for insert/replace)"
                    }),
                ),
                (
                    "source".to_string(),
                    json!({
                        "type": "string",
                        "description": "Cell content (for insert/replace)"
                    }),
                ),
            ]),
            required: vec![
                "file_path".to_string(),
                "command".to_string(),
                "cell_number".to_string(),
            ],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'file_path'".to_string()))?;
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'command'".to_string()))?;
        let cell_number = input
            .get("cell_number")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'cell_number'".to_string()))?
            as usize;

        let resolved = Path::new(&context.working_dir).join(file_path);
        let resolved_str = resolved.to_string_lossy().to_string();

        let content = match tokio::fs::read_to_string(&resolved).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(format!("Error: {}", e))),
        };

        let mut notebook: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => return Ok(ToolResult::error(format!("Error: Invalid JSON: {}", e))),
        };

        let cells = match notebook.get_mut("cells").and_then(|c| c.as_array_mut()) {
            Some(c) => c,
            None => return Ok(ToolResult::error("Error: Invalid notebook format".to_string())),
        };

        let cell_type = input
            .get("cell_type")
            .and_then(|v| v.as_str())
            .unwrap_or("code");
        let source = input
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Split source into lines matching Jupyter format (each line ends with \n except last)
        let source_lines: Vec<Value> = source
            .split('\n')
            .enumerate()
            .map(|(i, line)| {
                let total = source.split('\n').count();
                if i < total - 1 {
                    json!(format!("{}\n", line))
                } else {
                    json!(line)
                }
            })
            .collect();

        match command {
            "insert" => {
                let mut new_cell = json!({
                    "cell_type": cell_type,
                    "source": source_lines,
                    "metadata": {}
                });
                if cell_type != "markdown" {
                    new_cell["outputs"] = json!([]);
                    new_cell["execution_count"] = Value::Null;
                }
                cells.insert(cell_number, new_cell);
            }
            "replace" => {
                if cell_number >= cells.len() {
                    return Ok(ToolResult::error(format!(
                        "Error: Cell {} does not exist",
                        cell_number
                    )));
                }
                cells[cell_number]["source"] = json!(source_lines);
                if !cell_type.is_empty() {
                    cells[cell_number]["cell_type"] = json!(cell_type);
                }
            }
            "delete" => {
                if cell_number >= cells.len() {
                    return Ok(ToolResult::error(format!(
                        "Error: Cell {} does not exist",
                        cell_number
                    )));
                }
                cells.remove(cell_number);
            }
            _ => {
                return Ok(ToolResult::error(format!(
                    "Error: Unknown command '{}'",
                    command
                )));
            }
        }

        let output = serde_json::to_string_pretty(&notebook)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to serialize: {}", e)))?;

        if let Err(e) = tokio::fs::write(&resolved, output).await {
            return Ok(ToolResult::error(format!("Error writing file: {}", e)));
        }

        Ok(ToolResult::text(format!(
            "Notebook {}: cell {} in {}",
            command, cell_number, resolved_str
        )))
    }
}
