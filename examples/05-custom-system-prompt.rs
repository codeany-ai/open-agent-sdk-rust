// Example 5: Custom System Prompt
//
// Creates a specialized code reviewer agent with a custom system prompt.
//
// Run: cargo run --example 05-custom-system-prompt

use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;

#[tokio::main]
async fn main() {
    println!("--- Example 5: Custom System Prompt ---");

    let mut agent = Agent::new(AgentOptions {
        system_prompt: Some(
            "You are a senior code reviewer. When analyzing code, focus on:\n\
             1. Security vulnerabilities\n\
             2. Performance issues\n\
             3. Maintainability concerns\n\
             Be concise and use bullet points."
                .to_string(),
        ),
        max_turns: Some(10),
        ..Default::default()
    })
    .await
    .unwrap();

    let (mut rx, handle) = agent
        .query("Read src/lib.rs and give a brief code review.")
        .await;

    while let Some(event) = rx.recv().await {
        match event {
            SDKMessage::Assistant { message, .. } => {
                let text = types::extract_text(&message);
                if !text.is_empty() {
                    print!("{}", text);
                }
            }
            SDKMessage::Result { .. } => {
                println!("\n\n--- Done ---");
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
