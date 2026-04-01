// Example 2: Multi-Tool Orchestration
//
// The agent autonomously uses Glob, Bash, and Read to find files,
// count lines, and provide a summary.
//
// Run: cargo run --example 02-multi-tool

use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;

#[tokio::main]
async fn main() {
    println!("--- Example 2: Multi-Tool ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(15),
        ..Default::default()
    })
    .await
    .unwrap();

    let (mut rx, handle) = agent
        .query(
            "Find all .rs files in the src/ directory, count the total lines of code \
             with wc, and give me a brief summary.",
        )
        .await;

    while let Some(event) = rx.recv().await {
        match event {
            SDKMessage::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        types::ContentBlock::ToolUse { name, input, .. } => {
                            let input_str = serde_json::to_string(input).unwrap_or_default();
                            let truncated = if input_str.len() > 80 {
                                format!("{}...", &input_str[..80])
                            } else {
                                input_str
                            };
                            println!("[{}] {}", name, truncated);
                        }
                        types::ContentBlock::Text { text } => {
                            if !text.trim().is_empty() {
                                println!("\n{}", text);
                            }
                        }
                        _ => {}
                    }
                }
            }
            SDKMessage::Result { usage, .. } => {
                println!("\n--- Done ---");
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
