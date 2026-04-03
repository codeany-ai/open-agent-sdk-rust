use crate::types::{ApiToolParam, ContentBlock, Message, SystemBlock, ThinkingConfig, Usage};
use async_trait::async_trait;

/// API type identifier.
#[derive(Debug, Clone, PartialEq)]
pub enum ApiType {
    AnthropicMessages,
    OpenAICompletions,
}

/// Normalized response from any LLM provider.
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub message: Message,
    pub usage: Usage,
    pub stop_reason: Option<String>,
}

/// Configuration passed to provider for each request.
#[derive(Debug, Clone)]
pub struct ProviderRequest<'a> {
    pub model: &'a str,
    pub max_tokens: u64,
    pub messages: &'a [Message],
    pub system: Option<Vec<SystemBlock>>,
    pub tools: Option<Vec<ApiToolParam>>,
    pub thinking: Option<ThinkingConfig>,
}

/// Trait that all LLM providers must implement.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn api_type(&self) -> ApiType;

    async fn create_message(
        &self,
        request: ProviderRequest<'_>,
    ) -> Result<ProviderResponse, super::ApiError>;
}

/// Detect API type from model name using heuristics (matches TS SDK).
pub fn detect_api_type(model: &str, env_api_type: Option<&str>) -> ApiType {
    // 1. Explicit env override
    if let Some(t) = env_api_type {
        match t {
            "openai-completions" => return ApiType::OpenAICompletions,
            "anthropic-messages" => return ApiType::AnthropicMessages,
            _ => {}
        }
    }

    // 2. Also check CODEANY_API_TYPE env var
    if let Ok(t) = std::env::var("CODEANY_API_TYPE") {
        match t.as_str() {
            "openai-completions" => return ApiType::OpenAICompletions,
            "anthropic-messages" => return ApiType::AnthropicMessages,
            _ => {}
        }
    }

    // 3. Model name heuristics
    let m = model.to_lowercase();
    if m.contains("gpt-")
        || m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4")
        || m.contains("deepseek")
        || m.contains("qwen")
        || m.contains("yi-")
        || m.contains("glm")
        || m.contains("mistral")
        || m.contains("gemma")
        || m.contains("mimo")
        || m.contains("llama")
        || m.contains("gemini")
    {
        return ApiType::OpenAICompletions;
    }

    // Default: Anthropic
    ApiType::AnthropicMessages
}
