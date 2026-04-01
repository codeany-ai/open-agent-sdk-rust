// Example 9: Subagents
//
// Creates a specialized code-reviewer subagent with restricted tools.
//
// Run: cargo run --example 09-subagents

use open_agent_sdk::agent::SubagentDefinition;
use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    println!("--- Example 9: Subagents ---");

    let mut agents = HashMap::new();
    agents.insert(
        "code-reviewer".to_string(),
        SubagentDefinition {
            description: "Expert code reviewer that analyzes code quality".to_string(),
            instructions: Some(
                "You are an expert code reviewer. Analyze code for:\n\
                 - Code quality and readability\n\
                 - Potential bugs or edge cases\n\
                 - Suggested improvements\n\
                 Be concise and constructive."
                    .to_string(),
            ),
            tools: Some(vec![
                "Read".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
            ]),
            model: None,
        },
    );

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(15),
        agents,
        ..Default::default()
    })
    .await
    .unwrap();

    let (mut rx, handle) = agent
        .query("Use the code-reviewer agent to review src/lib.rs")
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
