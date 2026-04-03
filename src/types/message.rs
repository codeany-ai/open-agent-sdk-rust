use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Message role in conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

/// A message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
}

/// Content block types used in messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Vec<ToolResultContentBlock>,
        #[serde(default)]
        is_error: bool,
    },

    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: Option<String>,
    },

    #[serde(rename = "image")]
    Image { source: ImageContentSource },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContentSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageContentSource },
}

/// System prompt block with optional cache control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub control_type: String,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            control_type: "ephemeral".to_string(),
        }
    }
}

/// Token usage information from API response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// SDK message types emitted during the agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SDKMessage {
    #[serde(rename = "system")]
    System { message: String },

    #[serde(rename = "assistant")]
    Assistant {
        message: Message,
        #[serde(default)]
        usage: Option<Usage>,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        tool_name: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },

    #[serde(rename = "result")]
    Result {
        #[serde(default)]
        text: String,
        #[serde(default)]
        usage: Usage,
        #[serde(default)]
        num_turns: u32,
        #[serde(default)]
        cost_usd: f64,
        #[serde(default)]
        duration_ms: u64,
        #[serde(default)]
        messages: Vec<Message>,
    },

    #[serde(rename = "partial_message")]
    PartialMessage { text: String },

    #[serde(rename = "compact_boundary")]
    CompactBoundary {
        #[serde(default)]
        summary: String,
    },

    #[serde(rename = "status")]
    Status { message: String },

    #[serde(rename = "task_notification")]
    TaskNotification {
        task_id: String,
        status: String,
        #[serde(default)]
        message: Option<String>,
    },

    #[serde(rename = "rate_limit")]
    RateLimit {
        #[serde(default)]
        retry_after_ms: u64,
        #[serde(default)]
        message: String,
    },

    #[serde(rename = "progress")]
    Progress { message: String },

    #[serde(rename = "error")]
    Error { message: String },
}

/// Query result returned by the blocking `prompt()` method.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub text: String,
    pub usage: Usage,
    pub num_turns: u32,
    pub cost_usd: f64,
    pub duration_ms: u64,
    pub messages: Vec<Message>,
}

/// Tool definition in API format (for sending to Claude).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolParam {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Extract text content from a message.
pub fn extract_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Extract tool use blocks from a message.
pub fn extract_tool_uses(message: &Message) -> Vec<(String, String, Value)> {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => {
                Some((id.clone(), name.clone(), input.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Thinking configuration for extended thinking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,
    #[serde(default)]
    pub budget_tokens: Option<u64>,
}

impl ThinkingConfig {
    pub fn enabled(budget_tokens: u64) -> Self {
        Self {
            thinking_type: "enabled".to_string(),
            budget_tokens: Some(budget_tokens),
        }
    }

    pub fn disabled() -> Self {
        Self {
            thinking_type: "disabled".to_string(),
            budget_tokens: None,
        }
    }
}
