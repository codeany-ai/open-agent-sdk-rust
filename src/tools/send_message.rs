use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

/// Shared mailbox for inter-agent communication.
pub type Mailbox = Arc<RwLock<HashMap<String, Vec<AgentMessage>>>>;

/// A message between agents.
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    pub timestamp: String,
    pub message_type: String, // "text", "shutdown_request", "shutdown_response", "plan_approval_response"
}

/// Create a new empty mailbox.
pub fn new_mailbox() -> Mailbox {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Read and drain messages for an agent.
pub async fn read_mailbox(mailbox: &Mailbox, agent_name: &str) -> Vec<AgentMessage> {
    let mut mb = mailbox.write().await;
    mb.remove(agent_name).unwrap_or_default()
}

/// Write a message to an agent's mailbox.
pub async fn write_to_mailbox(mailbox: &Mailbox, agent_name: &str, message: AgentMessage) {
    let mut mb = mailbox.write().await;
    mb.entry(agent_name.to_string())
        .or_insert_with(Vec::new)
        .push(message);
}

/// SendMessageTool - Send a message to another agent or teammate.
pub struct SendMessageTool {
    mailbox: Mailbox,
}

impl SendMessageTool {
    pub fn new(mailbox: Mailbox) -> Self {
        Self { mailbox }
    }
}

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "SendMessage"
    }

    fn description(&self) -> &str {
        "Send a message to another agent or teammate. Supports plain text and structured protocol messages."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "to".to_string(),
                    json!({
                        "type": "string",
                        "description": "Recipient agent name or ID. Use \"*\" for broadcast."
                    }),
                ),
                (
                    "content".to_string(),
                    json!({
                        "type": "string",
                        "description": "Message content"
                    }),
                ),
                (
                    "type".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["text", "shutdown_request", "shutdown_response", "plan_approval_response"],
                        "description": "Message type (default: text)"
                    }),
                ),
            ]),
            required: vec!["to".to_string(), "content".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    async fn call(&self, input: Value, _context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let to = input
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'to'".to_string()))?;
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'content'".to_string()))?;
        let msg_type = input
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let timestamp = chrono::Utc::now().to_rfc3339();

        let message = AgentMessage {
            from: "self".to_string(),
            to: to.to_string(),
            content: content.to_string(),
            timestamp,
            message_type: msg_type.to_string(),
        };

        if to == "*" {
            // Broadcast to all known mailboxes
            let mb = self.mailbox.read().await;
            let names: Vec<String> = mb.keys().cloned().collect();
            drop(mb);

            for name in &names {
                let mut broadcast_msg = message.clone();
                broadcast_msg.to = name.clone();
                write_to_mailbox(&self.mailbox, name, broadcast_msg).await;
            }

            Ok(ToolResult::text("Message broadcast to all agents"))
        } else {
            write_to_mailbox(&self.mailbox, to, message).await;
            Ok(ToolResult::text(format!("Message sent to {}", to)))
        }
    }
}
