use crate::api::ApiClient;
use crate::context;
use crate::costtracker::CostTracker;
use crate::tools::{self, ToolRegistry};
use crate::types::*;
use crate::utils::{compact, messages as msg_utils, retry};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Run the main agentic loop.
pub(crate) async fn run_loop(
    api_client: ApiClient,
    mut messages: Vec<Message>,
    registry: Arc<ToolRegistry>,
    cwd: &str,
    system_prompt: Option<&str>,
    append_system_prompt: Option<&str>,
    max_turns: u32,
    max_budget_usd: Option<f64>,
    cost_tracker: &CostTracker,
    thinking: Option<ThinkingConfig>,
    max_tokens: Option<u64>,
    can_use_tool: Option<&CanUseToolFn>,
    tx: mpsc::Sender<SDKMessage>,
) -> Result<Vec<Message>, String> {
    let tool_context = ToolUseContext::new(cwd.to_string());

    // Build system prompt blocks
    let system_blocks = context::build_system_blocks(cwd, system_prompt, append_system_prompt);

    // Build tool definitions for API
    let api_tools: Vec<ApiToolParam> = registry
        .all()
        .iter()
        .map(|tool| {
            let schema = tool.input_schema();
            ApiToolParam {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: serde_json::to_value(&schema).unwrap_or_default(),
            }
        })
        .collect();

    let retry_config = retry::RetryConfig::default();
    let mut num_turns: u32 = 0;
    let mut total_usage = Usage::default();

    // Send system message
    let _ = tx
        .send(SDKMessage::System {
            message: format!(
                "Agent started with {} tools ({})",
                api_tools.len(),
                match api_client.api_type() {
                    crate::api::ApiType::AnthropicMessages => "anthropic",
                    crate::api::ApiType::OpenAICompletions => "openai",
                }
            ),
        })
        .await;

    loop {
        // Check turn limit
        num_turns += 1;
        if num_turns > max_turns {
            let _ = tx
                .send(SDKMessage::Progress {
                    message: format!("Reached max turns ({})", max_turns),
                })
                .await;
            break;
        }

        // Check budget
        if let Some(budget) = max_budget_usd {
            let current_cost = cost_tracker.total_cost().await;
            if current_cost >= budget {
                let _ = tx
                    .send(SDKMessage::Progress {
                        message: format!(
                            "Budget exceeded: ${:.4} >= ${:.4}",
                            current_cost, budget
                        ),
                    })
                    .await;
                break;
            }
        }

        // Check if auto-compaction is needed
        if compact::should_auto_compact(&messages, api_client.model()) {
            messages = compact::micro_compact_messages(&messages);
        }

        // Normalize messages
        let normalized = msg_utils::normalize_messages(&messages);

        // Call the API via provider abstraction with retry
        let api_client_ref = &api_client;
        let system_blocks_ref = &system_blocks;
        let api_tools_ref = &api_tools;
        let thinking_ref = &thinking;

        let start = std::time::Instant::now();

        let response = retry::with_retry(&retry_config, || async {
            api_client_ref
                .create_message(
                    &normalized,
                    Some(system_blocks_ref.clone()),
                    Some(api_tools_ref.clone()),
                    max_tokens,
                    thinking_ref.clone(),
                )
                .await
        })
        .await;

        let provider_response = match response {
            Ok(r) => r,
            Err(e) => {
                if retry::is_auth_error(&e) {
                    return Err(format!("Authentication error: {}. Check your API key.", e));
                }
                return Err(format!("API error: {}", e));
            }
        };

        let api_duration = start.elapsed().as_millis() as u64;
        cost_tracker.add_api_duration(api_duration).await;
        cost_tracker
            .add_usage(api_client.model(), &provider_response.usage)
            .await;

        let usage = &provider_response.usage;
        let assistant_msg = provider_response.message;
        let stop_reason = provider_response.stop_reason;

        // Accumulate usage
        total_usage.input_tokens += usage.input_tokens;
        total_usage.output_tokens += usage.output_tokens;
        total_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
        total_usage.cache_read_input_tokens += usage.cache_read_input_tokens;

        // Send assistant message event
        let _ = tx
            .send(SDKMessage::Assistant {
                message: assistant_msg.clone(),
                usage: Some(usage.clone()),
            })
            .await;

        messages.push(assistant_msg.clone());

        // Check for tool use
        let tool_uses = extract_tool_uses(&assistant_msg);

        if tool_uses.is_empty() {
            // No tool calls - we're done (end_turn or max_tokens)
            break;
        }

        // Execute tools
        let tool_start = std::time::Instant::now();
        let results =
            tools::execute_tools(&assistant_msg, &registry, &tool_context, can_use_tool).await;
        let tool_duration = tool_start.elapsed().as_millis() as u64;
        cost_tracker.add_tool_duration(tool_duration).await;

        // Send tool result events
        for (id, name, result) in &results {
            let content_text = result.get_text();
            let _ = tx
                .send(SDKMessage::ToolResult {
                    tool_use_id: id.clone(),
                    tool_name: name.clone(),
                    content: content_text,
                    is_error: result.is_error,
                })
                .await;
        }

        // Build tool results message and add to conversation
        let tool_results_msg = tools::build_tool_results_message(&results);
        messages.push(tool_results_msg);

        // Check stop reason
        if stop_reason.as_deref() == Some("end_turn") && tool_uses.is_empty() {
            break;
        }
    }

    // Calculate final cost
    let total_cost = cost_tracker.total_cost().await;

    // Extract final text
    let final_text = messages
        .iter()
        .rev()
        .find(|m| m.role == MessageRole::Assistant)
        .map(|m| extract_text(m))
        .unwrap_or_default();

    // Send result event
    let _ = tx
        .send(SDKMessage::Result {
            text: final_text,
            usage: total_usage,
            num_turns,
            cost_usd: total_cost,
            duration_ms: 0,
            messages: messages.clone(),
        })
        .await;

    Ok(messages)
}
