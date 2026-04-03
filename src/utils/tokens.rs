use crate::types::{Message, Usage};

/// Estimate token count from text (~4 characters per token).
pub fn estimate_tokens(text: &str) -> u64 {
    (text.len() as f64 / 4.0).ceil() as u64
}

/// Estimate total tokens for a list of messages.
pub fn estimate_messages_tokens(messages: &[Message]) -> u64 {
    messages
        .iter()
        .map(|m| {
            let content_str = serde_json::to_string(&m.content).unwrap_or_default();
            estimate_tokens(&content_str) + 4 // overhead per message
        })
        .sum()
}

/// Get context window size for a model.
pub fn get_context_window_size(model: &str) -> u64 {
    match model {
        // Anthropic models
        m if m.contains("opus") && m.contains("1m") => 1_000_000,
        m if m.contains("opus") => 200_000,
        m if m.contains("sonnet") => 200_000,
        m if m.contains("haiku") => 200_000,
        // OpenAI models
        m if m.starts_with("o1") => 200_000,
        m if m.starts_with("o3") => 200_000,
        m if m.starts_with("o4-mini") => 200_000,
        m if m.starts_with("gpt-4o-mini") => 128_000,
        m if m.starts_with("gpt-4o") => 128_000,
        // DeepSeek models
        m if m.starts_with("deepseek") => 128_000,
        _ => 200_000,
    }
}

/// Get auto-compact threshold for a model (context window - 13k tokens).
pub fn get_auto_compact_threshold(model: &str) -> u64 {
    let window = get_context_window_size(model);
    window.saturating_sub(13_000)
}

/// Model pricing per million tokens.
pub struct ModelPricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// Get pricing for a model.
pub fn get_model_pricing(model: &str) -> ModelPricing {
    match model {
        // Anthropic models
        m if m.contains("opus") => ModelPricing {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        m if m.contains("sonnet") => ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        m if m.contains("haiku") => ModelPricing {
            input: 0.8,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        // OpenAI models
        m if m.starts_with("gpt-4o-mini") => ModelPricing {
            input: 0.15,
            output: 0.60,
            cache_read: 0.075,
            cache_write: 0.15,
        },
        m if m.starts_with("gpt-4o") => ModelPricing {
            input: 2.50,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 2.50,
        },
        m if m.starts_with("o1") => ModelPricing {
            input: 15.0,
            output: 60.0,
            cache_read: 7.5,
            cache_write: 15.0,
        },
        m if m.starts_with("o3") => ModelPricing {
            input: 10.0,
            output: 40.0,
            cache_read: 5.0,
            cache_write: 10.0,
        },
        m if m.starts_with("o4-mini") => ModelPricing {
            input: 1.10,
            output: 4.40,
            cache_read: 0.55,
            cache_write: 1.10,
        },
        // DeepSeek models
        m if m.starts_with("deepseek-reasoner") => ModelPricing {
            input: 0.55,
            output: 2.19,
            cache_read: 0.14,
            cache_write: 0.55,
        },
        m if m.starts_with("deepseek-chat") => ModelPricing {
            input: 0.27,
            output: 1.10,
            cache_read: 0.07,
            cache_write: 0.27,
        },
        // Default (sonnet-like pricing)
        _ => ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
    }
}

/// Estimate cost in USD for given usage and model.
pub fn estimate_cost(model: &str, usage: &Usage) -> f64 {
    let pricing = get_model_pricing(model);
    let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * pricing.input;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * pricing.output;
    let cache_read_cost =
        (usage.cache_read_input_tokens as f64 / 1_000_000.0) * pricing.cache_read;
    let cache_write_cost =
        (usage.cache_creation_input_tokens as f64 / 1_000_000.0) * pricing.cache_write;

    input_cost + output_cost + cache_read_cost + cache_write_cost
}
