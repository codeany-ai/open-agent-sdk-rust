use crate::types::{
    ApiToolParam, ContentBlock, ImageContentSource, Message, MessageRole,
    SystemBlock, ThinkingConfig, Usage,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_TIMEOUT_MS: u64 = 600_000; // 10 minutes

/// Model configuration with context window and output limits.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub context_window: u64,
    pub max_output_tokens: u64,
}

/// Get model configuration for a given model ID.
pub fn get_model_config(model: &str) -> ModelConfig {
    match model {
        m if m.contains("opus") => ModelConfig {
            context_window: 200_000,
            max_output_tokens: 32_000,
        },
        m if m.contains("sonnet") => ModelConfig {
            context_window: 200_000,
            max_output_tokens: 16_000,
        },
        m if m.contains("haiku") => ModelConfig {
            context_window: 200_000,
            max_output_tokens: 8_192,
        },
        _ => ModelConfig {
            context_window: 200_000,
            max_output_tokens: 16_000,
        },
    }
}

/// API client for the Anthropic Messages API.
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    custom_headers: HashMap<String, String>,
}

/// Request body for the Messages API.
#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u64,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<Vec<SystemBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiToolParam>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: Value,
}

/// Streaming event from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub message: Option<Value>,
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub content_block: Option<Value>,
    #[serde(default)]
    pub delta: Option<Value>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Complete API response (non-streaming).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse {
    pub id: String,
    pub content: Vec<Value>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

/// Error from the API.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP error: {status} - {message}")]
    HttpError { status: u16, message: String },

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Rate limit exceeded")]
    RateLimitError,

    #[error("Prompt too long: {0}")]
    PromptTooLong(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Request timeout")]
    Timeout,
}

impl ApiClient {
    pub fn new(api_key: Option<String>, base_url: Option<String>, model: Option<String>) -> Self {
        let api_key = api_key
            .or_else(|| std::env::var("CODEANY_API_KEY").ok())
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .unwrap_or_default();

        let base_url = base_url
            .or_else(|| std::env::var("CODEANY_BASE_URL").ok())
            .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let model = model
            .or_else(|| std::env::var("CODEANY_MODEL").ok())
            .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
            .unwrap_or_else(|| "sonnet-4-6".to_string());

        let timeout_ms: u64 = std::env::var("API_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS);

        let client = Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .expect("Failed to create HTTP client");

        let mut custom_headers = HashMap::new();
        if let Ok(headers_str) = std::env::var("CODEANY_CUSTOM_HEADERS") {
            for pair in headers_str.split(',') {
                if let Some((key, value)) = pair.split_once(':') {
                    custom_headers.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }

        Self {
            client,
            api_key,
            base_url,
            model,
            custom_headers,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn model_config(&self) -> ModelConfig {
        get_model_config(&self.model)
    }

    /// Send a streaming messages request. Returns the raw response for SSE processing.
    pub async fn create_message_stream(
        &self,
        messages: &[Message],
        system: Option<Vec<SystemBlock>>,
        tools: Option<Vec<ApiToolParam>>,
        max_tokens: Option<u64>,
        thinking: Option<ThinkingConfig>,
    ) -> Result<reqwest::Response, ApiError> {
        let model_config = self.model_config();
        let max_tokens = max_tokens.unwrap_or(model_config.max_output_tokens);

        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                };
                ApiMessage {
                    role: role.to_string(),
                    content: serde_json::to_value(&m.content).unwrap_or(Value::Array(vec![])),
                }
            })
            .collect();

        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens,
            messages: api_messages,
            system,
            tools: if tools.as_ref().map_or(true, |t| t.is_empty()) {
                None
            } else {
                tools
            },
            stream: true,
            thinking,
            metadata: None,
        };

        let mut req_builder = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", "prompt-caching-2024-07-31")
            .header("content-type", "application/json");

        for (key, value) in &self.custom_headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ApiError::Timeout
                } else {
                    ApiError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status().as_u16();
        if status == 401 {
            return Err(ApiError::AuthError("Invalid API key".to_string()));
        }
        if status == 429 {
            return Err(ApiError::RateLimitError);
        }
        if status >= 400 {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            if body.contains("prompt is too long") {
                return Err(ApiError::PromptTooLong(body));
            }
            return Err(ApiError::HttpError {
                status,
                message: body,
            });
        }

        Ok(response)
    }

    /// Parse SSE events from a streaming response into content blocks and usage.
    pub async fn parse_stream(
        response: reqwest::Response,
    ) -> Result<(Message, Usage, Option<String>), ApiError> {
        let body = response
            .text()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut usage = Usage::default();
        let mut stop_reason: Option<String> = None;

        // Track content blocks being built
        let mut current_blocks: HashMap<usize, Value> = HashMap::new();

        for line in body.lines() {
            let line = line.trim();
            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data == "[DONE]" {
                break;
            }

            let event: StreamEvent = match serde_json::from_str(data) {
                Ok(e) => e,
                Err(_) => continue,
            };

            match event.event_type.as_str() {
                "message_start" => {
                    if let Some(msg) = &event.message {
                        if let Some(u) = msg.get("usage") {
                            if let Ok(u) = serde_json::from_value::<Usage>(u.clone()) {
                                usage.input_tokens = u.input_tokens;
                                usage.cache_creation_input_tokens = u.cache_creation_input_tokens;
                                usage.cache_read_input_tokens = u.cache_read_input_tokens;
                            }
                        }
                    }
                }
                "content_block_start" => {
                    if let (Some(idx), Some(block)) = (event.index, &event.content_block) {
                        current_blocks.insert(idx, block.clone());
                    }
                }
                "content_block_delta" => {
                    if let (Some(idx), Some(delta)) = (event.index, &event.delta) {
                        let delta_type = delta
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    let block = current_blocks.entry(idx).or_insert_with(|| {
                                        serde_json::json!({"type": "text", "text": ""})
                                    });
                                    if let Some(existing) = block.get("text").and_then(|t| t.as_str())
                                    {
                                        let new_text = format!("{}{}", existing, text);
                                        block["text"] = Value::String(new_text);
                                    }
                                }
                            }
                            "input_json_delta" => {
                                if let Some(partial) =
                                    delta.get("partial_json").and_then(|t| t.as_str())
                                {
                                    let block = current_blocks.entry(idx).or_insert_with(|| {
                                        serde_json::json!({"type": "tool_use", "id": "", "name": "", "input": {}})
                                    });
                                    let existing = block
                                        .get("_partial_json")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("");
                                    let new_json = format!("{}{}", existing, partial);
                                    block["_partial_json"] = Value::String(new_json);
                                }
                            }
                            "thinking_delta" => {
                                if let Some(thinking) =
                                    delta.get("thinking").and_then(|t| t.as_str())
                                {
                                    let block = current_blocks.entry(idx).or_insert_with(|| {
                                        serde_json::json!({"type": "thinking", "thinking": ""})
                                    });
                                    if let Some(existing) =
                                        block.get("thinking").and_then(|t| t.as_str())
                                    {
                                        let new_text = format!("{}{}", existing, thinking);
                                        block["thinking"] = Value::String(new_text);
                                    }
                                }
                            }
                            "signature_delta" => {
                                if let Some(sig) =
                                    delta.get("signature").and_then(|t| t.as_str())
                                {
                                    if let Some(block) = current_blocks.get_mut(&idx) {
                                        block["signature"] = Value::String(sig.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "content_block_stop" => {
                    if let Some(idx) = event.index {
                        if let Some(block) = current_blocks.remove(&idx) {
                            if let Some(content_block) = parse_content_block(block) {
                                content_blocks.push(content_block);
                            }
                        }
                    }
                }
                "message_delta" => {
                    if let Some(delta) = &event.delta {
                        if let Some(sr) = delta.get("stop_reason").and_then(|s| s.as_str()) {
                            stop_reason = Some(sr.to_string());
                        }
                    }
                    if let Some(u) = &event.usage {
                        usage.output_tokens = u.output_tokens;
                    }
                }
                _ => {}
            }
        }

        // Flush remaining blocks
        let mut remaining: Vec<(usize, Value)> = current_blocks.into_iter().collect();
        remaining.sort_by_key(|(idx, _)| *idx);
        for (_, block) in remaining {
            if let Some(content_block) = parse_content_block(block) {
                content_blocks.push(content_block);
            }
        }

        let message = Message {
            role: MessageRole::Assistant,
            content: content_blocks,
        };

        Ok((message, usage, stop_reason))
    }
}

/// Parse a raw JSON value into a ContentBlock.
fn parse_content_block(mut block: Value) -> Option<ContentBlock> {
    let block_type = block.get("type")?.as_str()?.to_string();

    match block_type.as_str() {
        "text" => {
            let text = block.get("text")?.as_str()?.to_string();
            Some(ContentBlock::Text { text })
        }
        "tool_use" => {
            let id = block.get("id")?.as_str()?.to_string();
            let name = block.get("name")?.as_str()?.to_string();
            // Parse accumulated partial JSON
            let input = if let Some(partial) = block.get("_partial_json").and_then(|p| p.as_str())
            {
                serde_json::from_str(partial).unwrap_or(Value::Object(serde_json::Map::new()))
            } else {
                block
                    .get("input")
                    .cloned()
                    .unwrap_or(Value::Object(serde_json::Map::new()))
            };
            // Clean up our internal field
            if let Some(obj) = block.as_object_mut() {
                obj.remove("_partial_json");
            }
            Some(ContentBlock::ToolUse { id, name, input })
        }
        "thinking" => {
            let thinking = block.get("thinking")?.as_str()?.to_string();
            let signature = block
                .get("signature")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            Some(ContentBlock::Thinking {
                thinking,
                signature,
            })
        }
        "image" => {
            let source = block.get("source")?;
            let source_type = source.get("type")?.as_str()?.to_string();
            let media_type = source.get("media_type")?.as_str()?.to_string();
            let data = source.get("data")?.as_str()?.to_string();
            Some(ContentBlock::Image {
                source: ImageContentSource {
                    source_type,
                    media_type,
                    data,
                },
            })
        }
        _ => None,
    }
}

/// Check if an error is retryable.
pub fn is_retryable_error(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::RateLimitError
            | ApiError::Timeout
            | ApiError::NetworkError(_)
            | ApiError::HttpError { status: 500..=599, .. }
    )
}

/// Check if an error is an auth error.
pub fn is_auth_error(error: &ApiError) -> bool {
    matches!(error, ApiError::AuthError(_))
}
