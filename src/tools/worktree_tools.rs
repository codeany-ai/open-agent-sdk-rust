use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// Info about an active worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub original_cwd: String,
}

/// Shared worktree tracker.
pub type WorktreeStore = Arc<RwLock<HashMap<String, WorktreeInfo>>>;

pub fn new_worktree_store() -> WorktreeStore {
    Arc::new(RwLock::new(HashMap::new()))
}

// ============================================================================
// EnterWorktreeTool
// ============================================================================

pub struct EnterWorktreeTool {
    store: WorktreeStore,
}

impl EnterWorktreeTool {
    pub fn new(store: WorktreeStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str {
        "EnterWorktree"
    }

    fn description(&self) -> &str {
        "Create an isolated git worktree for parallel work. The agent will work in the worktree without affecting the main working tree."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "branch".to_string(),
                    json!({ "type": "string", "description": "Branch name for the worktree (auto-generated if not provided)" }),
                ),
                (
                    "path".to_string(),
                    json!({ "type": "string", "description": "Path for the worktree (auto-generated if not provided)" }),
                ),
            ]),
            required: Vec::new(),
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        // Check if we're in a git repo
        let check = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(&context.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        if check.is_err() || !check.unwrap().status.success() {
            return Ok(ToolResult::error("Not in a git repository"));
        }

        let now = chrono::Utc::now().timestamp_millis();
        let branch = input
            .get("branch")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("worktree-{}", now));

        let worktree_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                let parent = std::path::Path::new(&context.working_dir)
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("/tmp"));
                parent
                    .join(format!(".worktree-{}", branch))
                    .to_string_lossy()
                    .to_string()
            });

        // Create branch if it doesn't exist (ignore errors if it already exists)
        let _ = Command::new("git")
            .args(["branch", &branch])
            .current_dir(&context.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        // Create worktree
        let result = Command::new("git")
            .args(["worktree", "add", &worktree_path, &branch])
            .current_dir(&context.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                let id = uuid::Uuid::new_v4().to_string();
                let info = WorktreeInfo {
                    path: worktree_path.clone(),
                    branch: branch.clone(),
                    original_cwd: context.working_dir.clone(),
                };

                let mut store = self.store.write().await;
                store.insert(id.clone(), info);

                Ok(ToolResult::text(format!(
                    "Worktree created:\n  ID: {}\n  Path: {}\n  Branch: {}\n\nYou are now working in the isolated worktree.",
                    id, worktree_path, branch
                )))
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Ok(ToolResult::error(format!(
                    "Error creating worktree: {}",
                    stderr
                )))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Error creating worktree: {}",
                e
            ))),
        }
    }
}

// ============================================================================
// ExitWorktreeTool
// ============================================================================

pub struct ExitWorktreeTool {
    store: WorktreeStore,
}

impl ExitWorktreeTool {
    pub fn new(store: WorktreeStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str {
        "ExitWorktree"
    }

    fn description(&self) -> &str {
        "Exit and optionally remove a git worktree. Use \"keep\" to preserve changes or \"remove\" to clean up."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "id".to_string(),
                    json!({ "type": "string", "description": "Worktree ID" }),
                ),
                (
                    "action".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["keep", "remove"],
                        "description": "Whether to keep or remove the worktree (default: remove)"
                    }),
                ),
            ]),
            required: vec!["id".to_string()],
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let id = input
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'id'".to_string()))?;

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("remove");

        let mut store = self.store.write().await;
        let worktree = match store.get(id) {
            Some(w) => w.clone(),
            None => return Ok(ToolResult::error(format!("Worktree not found: {}", id))),
        };

        if action == "remove" {
            // Remove worktree
            let result = Command::new("git")
                .args(["worktree", "remove", &worktree.path, "--force"])
                .current_dir(&worktree.original_cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;

            if let Ok(output) = result {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Ok(ToolResult::error(format!("Error: {}", stderr)));
                }
            }

            // Try to delete the branch (ignore errors)
            let _ = Command::new("git")
                .args(["branch", "-D", &worktree.branch])
                .current_dir(&worktree.original_cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;
        }

        store.remove(id);

        Ok(ToolResult::text(format!(
            "Worktree {}: {}",
            if action == "remove" {
                "removed"
            } else {
                "kept"
            },
            worktree.path
        )))
    }
}
