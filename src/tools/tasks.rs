use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// Task status values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// A task in the task store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
    #[serde(default)]
    pub output: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// In-memory task store shared across tools.
#[derive(Clone)]
pub struct TaskStore {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    counter: Arc<RwLock<u64>>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    async fn next_id(&self) -> String {
        let mut counter = self.counter.write().await;
        *counter += 1;
        format!("task_{}", counter)
    }
}

// --- TaskCreateTool ---

pub struct TaskCreateTool {
    store: TaskStore,
}

impl TaskCreateTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "TaskCreate"
    }

    fn description(&self) -> &str {
        "Create a new task for tracking work."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "subject".to_string(),
                    json!({ "type": "string", "description": "Task subject/title" }),
                ),
                (
                    "description".to_string(),
                    json!({ "type": "string", "description": "Optional task description" }),
                ),
                (
                    "owner".to_string(),
                    json!({ "type": "string", "description": "Optional task owner" }),
                ),
            ]),
            required: vec!["subject".to_string()],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let subject = input
            .get("subject")
            .and_then(|s| s.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'subject'".to_string()))?;

        let id = self.store.next_id().await;
        let now = chrono::Utc::now().to_rfc3339();

        let task = Task {
            id: id.clone(),
            subject: subject.to_string(),
            status: TaskStatus::Pending,
            owner: input.get("owner").and_then(|o| o.as_str()).map(String::from),
            description: input
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from),
            output: None,
            metadata: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
        };

        let mut tasks = self.store.tasks.write().await;
        tasks.insert(id.clone(), task);

        Ok(ToolResult::text(format!("Created task: {}", id)))
    }
}

// --- TaskGetTool ---

pub struct TaskGetTool {
    store: TaskStore,
}

impl TaskGetTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "TaskGet"
    }

    fn description(&self) -> &str {
        "Get a task by ID."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "id".to_string(),
                json!({ "type": "string", "description": "Task ID" }),
            )]),
            required: vec!["id".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let id = input
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

        let tasks = self.store.tasks.read().await;
        match tasks.get(id) {
            Some(task) => Ok(ToolResult::text(
                serde_json::to_string_pretty(task).unwrap_or_default(),
            )),
            None => Ok(ToolResult::error(format!("Task not found: {}", id))),
        }
    }
}

// --- TaskListTool ---

pub struct TaskListTool {
    store: TaskStore,
}

impl TaskListTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "TaskList"
    }

    fn description(&self) -> &str {
        "List all tasks."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema::default()
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, _input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let tasks = self.store.tasks.read().await;

        if tasks.is_empty() {
            return Ok(ToolResult::text("No tasks.".to_string()));
        }

        let mut task_list: Vec<&Task> = tasks.values().collect();
        task_list.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        let formatted: Vec<String> = task_list
            .iter()
            .map(|t| {
                format!(
                    "- [{}] {} ({:?}){}",
                    t.id,
                    t.subject,
                    t.status,
                    t.owner
                        .as_ref()
                        .map(|o| format!(" @{}", o))
                        .unwrap_or_default()
                )
            })
            .collect();

        Ok(ToolResult::text(formatted.join("\n")))
    }
}

// --- TaskUpdateTool ---

pub struct TaskUpdateTool {
    store: TaskStore,
}

impl TaskUpdateTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "TaskUpdate"
    }

    fn description(&self) -> &str {
        "Update a task's status or other properties."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "id".to_string(),
                    json!({ "type": "string", "description": "Task ID" }),
                ),
                (
                    "status".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "failed", "cancelled"],
                        "description": "New task status"
                    }),
                ),
                (
                    "owner".to_string(),
                    json!({ "type": "string", "description": "New task owner" }),
                ),
                (
                    "output".to_string(),
                    json!({ "type": "string", "description": "Task output/result" }),
                ),
            ]),
            required: vec!["id".to_string()],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let id = input
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

        let mut tasks = self.store.tasks.write().await;
        match tasks.get_mut(id) {
            Some(task) => {
                if let Some(status) = input.get("status").and_then(|s| s.as_str()) {
                    task.status = match status {
                        "pending" => TaskStatus::Pending,
                        "in_progress" => TaskStatus::InProgress,
                        "completed" => TaskStatus::Completed,
                        "failed" => TaskStatus::Failed,
                        "cancelled" => TaskStatus::Cancelled,
                        _ => {
                            return Ok(ToolResult::error(format!("Invalid status: {}", status)));
                        }
                    };
                }
                if let Some(owner) = input.get("owner").and_then(|o| o.as_str()) {
                    task.owner = Some(owner.to_string());
                }
                if let Some(output) = input.get("output").and_then(|o| o.as_str()) {
                    task.output = Some(output.to_string());
                }
                task.updated_at = chrono::Utc::now().to_rfc3339();

                Ok(ToolResult::text(format!("Updated task: {}", id)))
            }
            None => Ok(ToolResult::error(format!("Task not found: {}", id))),
        }
    }
}

// --- TaskStopTool ---

pub struct TaskStopTool {
    store: TaskStore,
}

impl TaskStopTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "TaskStop"
    }

    fn description(&self) -> &str {
        "Stop/cancel a running task."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "id".to_string(),
                    json!({ "type": "string", "description": "Task ID to stop" }),
                ),
                (
                    "reason".to_string(),
                    json!({ "type": "string", "description": "Reason for stopping" }),
                ),
            ]),
            required: vec!["id".to_string()],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let id = input
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

        let mut tasks = self.store.tasks.write().await;
        match tasks.get_mut(id) {
            Some(task) => {
                task.status = TaskStatus::Cancelled;
                task.updated_at = chrono::Utc::now().to_rfc3339();
                if let Some(reason) = input.get("reason").and_then(|r| r.as_str()) {
                    task.output = Some(format!("Stopped: {}", reason));
                }
                Ok(ToolResult::text(format!("Task stopped: {}", id)))
            }
            None => Ok(ToolResult::error(format!("Task not found: {}", id))),
        }
    }
}

// --- TaskOutputTool ---

pub struct TaskOutputTool {
    store: TaskStore,
}

impl TaskOutputTool {
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "TaskOutput"
    }

    fn description(&self) -> &str {
        "Get the output/result of a task."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "id".to_string(),
                json!({ "type": "string", "description": "Task ID" }),
            )]),
            required: vec!["id".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let id = input
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

        let tasks = self.store.tasks.read().await;
        match tasks.get(id) {
            Some(task) => {
                let output = task.output.as_deref().unwrap_or("(no output yet)");
                Ok(ToolResult::text(output))
            }
            None => Ok(ToolResult::error(format!("Task not found: {}", id))),
        }
    }
}
