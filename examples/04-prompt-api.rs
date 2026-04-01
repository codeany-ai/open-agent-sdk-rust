// Example 4: Blocking Prompt API
//
// Uses agent.prompt() for a quick one-shot query without streaming.
//
// Run: cargo run --example 04-prompt-api

use open_agent_sdk::{Agent, AgentOptions};

#[tokio::main]
async fn main() {
    println!("--- Example 4: Prompt API ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(5),
        ..Default::default()
    })
    .await
    .unwrap();

    match agent
        .prompt("Run 'rustc --version' and 'cargo --version' using Bash, and tell me the versions.")
        .await
    {
        Ok(result) => {
            println!("\nResponse: {}", result.text);
            println!("\nTurns: {}", result.num_turns);
            println!(
                "Tokens: {} in / {} out",
                result.usage.input_tokens, result.usage.output_tokens
            );
            println!("Duration: {}ms", result.duration_ms);
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    agent.close().await;
}
