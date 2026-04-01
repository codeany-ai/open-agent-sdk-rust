// Example 10: Permissions
//
// Read-only agent with AllowedTools restriction.
// Can only use Read, Glob, Grep — no write or execute.
//
// Run: cargo run --example 10-permissions

use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;

#[tokio::main]
async fn main() {
    println!("--- Example 10: Permissions ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(10),
        allowed_tools: Some(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ]),
        ..Default::default()
    })
    .await
    .unwrap();

    let (mut rx, handle) = agent
        .query("Review the code in src/lib.rs for best practices and potential improvements.")
        .await;

    while let Some(event) = rx.recv().await {
        match event {
            SDKMessage::Assistant { message, .. } => {
                let text = types::extract_text(&message);
                if !text.is_empty() {
                    print!("{}", text);
                }
            }
            SDKMessage::Result { cost_usd, .. } => {
                println!("\n\n--- Done (${:.4}) ---", cost_usd);
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
