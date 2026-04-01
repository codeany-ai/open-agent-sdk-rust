// Example 1: Simple Query with Streaming
//
// Demonstrates the basic Agent::new() + query() flow with
// real-time event streaming.
//
// Run: cargo run --example 01-simple-query

use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;

#[tokio::main]
async fn main() {
    println!("--- Example 1: Simple Query ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(10),
        ..Default::default()
    })
    .await
    .unwrap();

    let (mut rx, handle) = agent
        .query("Read Cargo.toml and tell me the project name and version in one sentence.")
        .await;

    while let Some(event) = rx.recv().await {
        match event {
            SDKMessage::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        types::ContentBlock::ToolUse { id, name, input } => {
                            let input_str = serde_json::to_string(input).unwrap_or_default();
                            let truncated = if input_str.len() > 80 {
                                format!("{}...", &input_str[..80])
                            } else {
                                input_str
                            };
                            println!("[Tool] {}({})", name, truncated);
                        }
                        types::ContentBlock::Text { text } => {
                            if !text.trim().is_empty() {
                                println!("\nAssistant: {}", text);
                            }
                        }
                        _ => {}
                    }
                }
            }
            SDKMessage::ToolResult {
                tool_name, content, ..
            } => {
                let truncated = if content.len() > 200 {
                    format!("{}...", &content[..200])
                } else {
                    content
                };
                println!("[Result: {}] {}", tool_name, truncated);
            }
            SDKMessage::Result {
                usage, ..
            } => {
                println!("\n--- Result ---");
                println!(
                    "Tokens: {} in / {} out",
                    usage.input_tokens, usage.output_tokens
                );
            }
            SDKMessage::Error { message } => {
                eprintln!("Error: {}", message);
            }
            _ => {}
        }
    }

    handle.await.unwrap();
    agent.close().await;
}
