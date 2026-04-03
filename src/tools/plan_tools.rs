use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// Shared plan mode state.
#[derive(Clone)]
pub struct PlanState {
    active: Arc<RwLock<bool>>,
    current_plan: Arc<RwLock<Option<String>>>,
}

impl PlanState {
    pub fn new() -> Self {
        Self {
            active: Arc::new(RwLock::new(false)),
            current_plan: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn is_active(&self) -> bool {
        *self.active.read().await
    }

    pub async fn get_plan(&self) -> Option<String> {
        self.current_plan.read().await.clone()
    }
}

// ============================================================================
// EnterPlanModeTool
// ============================================================================

pub struct EnterPlanModeTool {
    state: PlanState,
}

impl EnterPlanModeTool {
    pub fn new(state: PlanState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "EnterPlanMode"
    }

    fn description(&self) -> &str {
        "Enter plan/design mode for complex tasks. In plan mode, the agent focuses on designing the approach before executing."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
            additional_properties: Some(false),
        }
    }

    async fn call(&self, _input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let mut active = self.state.active.write().await;
        if *active {
            return Ok(ToolResult::text("Already in plan mode."));
        }

        *active = true;
        let mut plan = self.state.current_plan.write().await;
        *plan = None;

        Ok(ToolResult::text(
            "Entered plan mode. Design your approach before executing. Use ExitPlanMode when the plan is ready.",
        ))
    }
}

// ============================================================================
// ExitPlanModeTool
// ============================================================================

pub struct ExitPlanModeTool {
    state: PlanState,
}

impl ExitPlanModeTool {
    pub fn new(state: PlanState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "ExitPlanMode"
    }

    fn description(&self) -> &str {
        "Exit plan mode with a completed plan. The plan will be recorded and execution can proceed."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "plan".to_string(),
                    json!({ "type": "string", "description": "The completed plan" }),
                ),
                (
                    "approved".to_string(),
                    json!({ "type": "boolean", "description": "Whether the plan is approved for execution" }),
                ),
            ]),
            required: Vec::new(),
            additional_properties: Some(false),
        }
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let mut active = self.state.active.write().await;
        if !*active {
            return Ok(ToolResult::error("Not in plan mode."));
        }

        *active = false;

        let plan_text = input.get("plan").and_then(|v| v.as_str()).map(String::from);
        let approved = input
            .get("approved")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut current_plan = self.state.current_plan.write().await;
        *current_plan = plan_text.clone();

        let status = if approved {
            "approved"
        } else {
            "pending approval"
        };

        let mut msg = format!("Plan mode exited. Plan status: {}.", status);
        if let Some(ref plan) = plan_text {
            msg.push_str(&format!("\n\nPlan:\n{}", plan));
        }

        Ok(ToolResult::text(msg))
    }
}
