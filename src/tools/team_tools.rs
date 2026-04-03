use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// A team for multi-agent coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
    pub leader: String,
    pub tasks: Vec<String>,
    pub created_at: String,
    pub status: String, // "active" or "disbanded"
}

/// In-memory team store shared across tools.
#[derive(Clone)]
pub struct TeamStore {
    teams: Arc<RwLock<HashMap<String, Team>>>,
    counter: Arc<RwLock<u64>>,
}

impl TeamStore {
    pub fn new() -> Self {
        Self {
            teams: Arc::new(RwLock::new(HashMap::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    async fn next_id(&self) -> String {
        let mut counter = self.counter.write().await;
        *counter += 1;
        format!("team_{}", counter)
    }
}

// ============================================================================
// TeamCreateTool
// ============================================================================

pub struct TeamCreateTool {
    store: TeamStore,
}

impl TeamCreateTool {
    pub fn new(store: TeamStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        "TeamCreate"
    }

    fn description(&self) -> &str {
        "Create a multi-agent team for coordinated work. Assigns a lead and manages member composition."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "name".to_string(),
                    json!({ "type": "string", "description": "Team name" }),
                ),
                (
                    "members".to_string(),
                    json!({
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of agent/teammate names"
                    }),
                ),
                (
                    "task_description".to_string(),
                    json!({ "type": "string", "description": "Description of the team's mission" }),
                ),
            ]),
            required: vec!["name".to_string()],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'name'".to_string()))?;

        let members: Vec<String> = input
            .get("members")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let tasks: Vec<String> = input
            .get("task_description")
            .and_then(|v| v.as_str())
            .map(|t| vec![t.to_string()])
            .unwrap_or_default();

        let id = self.store.next_id().await;
        let now = chrono::Utc::now().to_rfc3339();

        let member_count = members.len();
        let team = Team {
            id: id.clone(),
            name: name.to_string(),
            members,
            leader: "self".to_string(),
            tasks,
            created_at: now,
            status: "active".to_string(),
        };

        let mut teams = self.store.teams.write().await;
        teams.insert(id.clone(), team);

        Ok(ToolResult::text(format!(
            "Team created: {} \"{}\" with {} members",
            id, name, member_count
        )))
    }
}

// ============================================================================
// TeamDeleteTool
// ============================================================================

pub struct TeamDeleteTool {
    store: TeamStore,
}

impl TeamDeleteTool {
    pub fn new(store: TeamStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TeamDeleteTool {
    fn name(&self) -> &str {
        "TeamDelete"
    }

    fn description(&self) -> &str {
        "Disband a team and clean up resources."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "id".to_string(),
                json!({ "type": "string", "description": "Team ID to disband" }),
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

        let mut teams = self.store.teams.write().await;
        match teams.remove(id) {
            Some(team) => Ok(ToolResult::text(format!("Team disbanded: {}", team.name))),
            None => Ok(ToolResult::error(format!("Team not found: {}", id))),
        }
    }
}
