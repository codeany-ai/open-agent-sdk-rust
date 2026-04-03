use crate::types::{ContentBlock, Message};
use crate::utils::tokens::{estimate_messages_tokens, get_auto_compact_threshold};

const MICRO_COMPACT_THRESHOLD: usize = 50_000; // chars per tool result

/// Check if auto-compaction is needed based on message token count.
pub fn should_auto_compact(messages: &[Message], model: &str) -> bool {
    let estimated = estimate_messages_tokens(messages);
    let threshold = get_auto_compact_threshold(model);
    estimated > threshold
}

/// Micro-compact: truncate large tool results in messages.
pub fn micro_compact_messages(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|msg| {
            let content = msg
                .content
                .iter()
                .map(|block| match block {
                    crate::types::ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let compacted_content: Vec<crate::types::ToolResultContentBlock> = content
                            .iter()
                            .map(|c| match c {
                                crate::types::ToolResultContentBlock::Text { text } => {
                                    if text.len() > MICRO_COMPACT_THRESHOLD {
                                        let truncated =
                                            format!("{}... (truncated)", &text[..MICRO_COMPACT_THRESHOLD]);
                                        crate::types::ToolResultContentBlock::Text {
                                            text: truncated,
                                        }
                                    } else {
                                        c.clone()
                                    }
                                }
                                _ => c.clone(),
                            })
                            .collect();
                        crate::types::ContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: compacted_content,
                            is_error: *is_error,
                        }
                    }
                    _ => block.clone(),
                })
                .collect();
            Message {
                role: msg.role.clone(),
                content,
            }
        })
        .collect()
}

/// Create a compact summary prompt for the LLM to summarize the conversation.
pub fn create_compact_prompt(messages: &[Message]) -> String {
    let message_count = messages.len();
    format!(
        "Please provide a concise summary of the conversation so far ({} messages). \
         Focus on: 1) What the user asked for, 2) What has been accomplished, \
         3) Key decisions made, 4) Current state and any pending work. \
         Keep the summary under 2000 tokens.",
        message_count
    )
}

/// Build the full compaction system prompt for the LLM summarizer.
///
/// This produces a system prompt that instructs the LLM to produce a structured
/// summary suitable for replacing the full conversation history.
pub fn build_compaction_prompt() -> String {
    r#"You are a conversation compaction assistant. Your job is to produce a concise, structured summary of a conversation between a user and an AI coding assistant.

The summary will REPLACE the original conversation, so it must preserve all information needed to continue the work seamlessly.

Output format:
1. **User Goal**: What the user originally asked for (1-2 sentences).
2. **Completed Work**: Bullet list of what has been done (files created/edited, commands run, decisions made).
3. **Current State**: The current state of the task - what's working, what's not.
4. **Pending Work**: Any remaining tasks or next steps that were discussed but not completed.
5. **Key Context**: Important facts, variable names, file paths, or constraints that would be needed to continue.

Rules:
- Be concise but complete. Do not omit file paths, function names, or error messages that are relevant.
- Do not include tool call details or raw tool output - summarize the results instead.
- Keep the total summary under 2000 tokens.
- If the conversation is already short, just summarize it briefly."#
        .to_string()
}

/// Perform full LLM-based conversation compaction.
///
/// Takes the conversation messages and returns a compacted conversation consisting of
/// a single user message (with the compaction prompt) ready to be sent to the LLM for
/// summarization. The caller is responsible for actually calling the LLM and using
/// the resulting summary.
///
/// Returns `(system_prompt, user_message)` where:
/// - `system_prompt` is the compaction system prompt
/// - `user_message` is the serialized conversation for the LLM to summarize
pub fn compact_conversation(messages: &[Message]) -> (String, String) {
    let system_prompt = build_compaction_prompt();

    // Serialize the conversation into a readable text format for the summarizer
    let mut conversation_text = String::new();
    for (i, msg) in messages.iter().enumerate() {
        let role = match &msg.role {
            crate::types::MessageRole::User => "User",
            crate::types::MessageRole::Assistant => "Assistant",
        };
        conversation_text.push_str(&format!("--- Message {} ({}) ---\n", i + 1, role));
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    // Truncate very long text blocks in the compaction input
                    if text.len() > 5000 {
                        conversation_text.push_str(&text[..5000]);
                        conversation_text.push_str("... [truncated]\n");
                    } else {
                        conversation_text.push_str(text);
                        conversation_text.push('\n');
                    }
                }
                ContentBlock::ToolUse { name, .. } => {
                    conversation_text.push_str(&format!("[Tool call: {}]\n", name));
                }
                ContentBlock::ToolResult { is_error, content, .. } => {
                    let preview = content
                        .iter()
                        .filter_map(|c| match c {
                            crate::types::ToolResultContentBlock::Text { text } => {
                                if text.len() > 500 {
                                    Some(format!("{}...", &text[..500]))
                                } else {
                                    Some(text.clone())
                                }
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let error_label = if *is_error { " (error)" } else { "" };
                    conversation_text
                        .push_str(&format!("[Tool result{}]: {}\n", error_label, preview));
                }
                ContentBlock::Thinking { thinking, .. } => {
                    if thinking.len() > 200 {
                        conversation_text
                            .push_str(&format!("[Thinking]: {}...\n", &thinking[..200]));
                    } else {
                        conversation_text.push_str(&format!("[Thinking]: {}\n", thinking));
                    }
                }
                ContentBlock::Image { .. } => {
                    conversation_text.push_str("[Image content]\n");
                }
            }
        }
        conversation_text.push('\n');
    }

    let user_message = format!(
        "Please summarize the following conversation:\n\n{}",
        conversation_text
    );

    (system_prompt, user_message)
}

/// Strip image blocks from messages, replacing them with a placeholder text block.
///
/// This is useful before compaction or when images are no longer needed and consume
/// excessive tokens in the context window.
pub fn strip_images_from_messages(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|msg| {
            let content = msg
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Image { .. } => ContentBlock::Text {
                        text: "[image removed during compaction]".to_string(),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let filtered_content: Vec<crate::types::ToolResultContentBlock> = content
                            .iter()
                            .map(|c| match c {
                                crate::types::ToolResultContentBlock::Image { .. } => {
                                    crate::types::ToolResultContentBlock::Text {
                                        text: "[image removed during compaction]".to_string(),
                                    }
                                }
                                other => other.clone(),
                            })
                            .collect();
                        ContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: filtered_content,
                            is_error: *is_error,
                        }
                    }
                    other => other.clone(),
                })
                .collect();
            Message {
                role: msg.role.clone(),
                content,
            }
        })
        .collect()
}
