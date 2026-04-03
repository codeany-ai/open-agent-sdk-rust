use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// A cron job definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub command: String,
    pub enabled: bool,
    pub created_at: String,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub next_run_at: Option<String>,
}

/// In-memory cron job store shared across cron tools.
#[derive(Clone)]
pub struct CronStore {
    jobs: Arc<RwLock<HashMap<String, CronJob>>>,
    counter: Arc<RwLock<u64>>,
}

impl CronStore {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    async fn next_id(&self) -> String {
        let mut counter = self.counter.write().await;
        *counter += 1;
        format!("cron_{}", counter)
    }
}

// --- CronCreateTool ---

pub struct CronCreateTool {
    store: CronStore,
}

impl CronCreateTool {
    pub fn new(store: CronStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CronCreateTool {
    fn name(&self) -> &str {
        "CronCreate"
    }

    fn description(&self) -> &str {
        "Create a scheduled recurring task (cron job). Supports cron expressions for scheduling."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "name".to_string(),
                    json!({ "type": "string", "description": "Job name" }),
                ),
                (
                    "schedule".to_string(),
                    json!({ "type": "string", "description": "Cron expression (e.g., \"*/5 * * * *\" for every 5 minutes)" }),
                ),
                (
                    "command".to_string(),
                    json!({ "type": "string", "description": "Command or prompt to execute" }),
                ),
            ]),
            required: vec![
                "name".to_string(),
                "schedule".to_string(),
                "command".to_string(),
            ],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'name'".to_string()))?;
        let schedule = input
            .get("schedule")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'schedule'".to_string()))?;
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'command'".to_string()))?;

        let id = self.store.next_id().await;
        let now = chrono::Utc::now().to_rfc3339();

        let job = CronJob {
            id: id.clone(),
            name: name.to_string(),
            schedule: schedule.to_string(),
            command: command.to_string(),
            enabled: true,
            created_at: now,
            last_run_at: None,
            next_run_at: None,
        };

        let mut jobs = self.store.jobs.write().await;
        jobs.insert(id.clone(), job.clone());

        Ok(ToolResult::text(format!(
            "Cron job created: {} \"{}\" schedule=\"{}\"",
            id, job.name, job.schedule
        )))
    }
}

// --- CronDeleteTool ---

pub struct CronDeleteTool {
    store: CronStore,
}

impl CronDeleteTool {
    pub fn new(store: CronStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CronDeleteTool {
    fn name(&self) -> &str {
        "CronDelete"
    }

    fn description(&self) -> &str {
        "Delete a scheduled cron job."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "id".to_string(),
                json!({ "type": "string", "description": "Cron job ID to delete" }),
            )]),
            required: vec!["id".to_string()],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let id = input
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

        let mut jobs = self.store.jobs.write().await;
        if jobs.remove(id).is_some() {
            Ok(ToolResult::text(format!("Cron job deleted: {}", id)))
        } else {
            Ok(ToolResult::error(format!("Cron job not found: {}", id)))
        }
    }
}

// --- CronListTool ---

pub struct CronListTool {
    store: CronStore,
}

impl CronListTool {
    pub fn new(store: CronStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &str {
        "CronList"
    }

    fn description(&self) -> &str {
        "List all scheduled cron jobs."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema::default()
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        _input: Value,
        _context: &ToolUseContext,
    ) -> Result<ToolResult, ToolError> {
        let jobs = self.store.jobs.read().await;

        if jobs.is_empty() {
            return Ok(ToolResult::text("No cron jobs scheduled."));
        }

        let mut job_list: Vec<&CronJob> = jobs.values().collect();
        job_list.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        let lines: Vec<String> = job_list
            .iter()
            .map(|j| {
                let status = if j.enabled { "enabled" } else { "disabled" };
                format!(
                    "[{}] {} \"{}\" schedule=\"{}\" command=\"{}\"",
                    j.id,
                    status,
                    j.name,
                    j.schedule,
                    if j.command.len() > 50 {
                        format!("{}...", &j.command[..50])
                    } else {
                        j.command.clone()
                    }
                )
            })
            .collect();

        Ok(ToolResult::text(lines.join("\n")))
    }
}
