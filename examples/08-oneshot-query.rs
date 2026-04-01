// Example 8: One-shot Query
//
// Quick one-shot agent query with restricted tools.
//
// Run: cargo run --example 08-oneshot-query

use open_agent_sdk::{Agent, AgentOptions};

#[tokio::main]
async fn main() {
    println!("--- Example 8: One-shot Query ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(5),
        allowed_tools: Some(vec!["Bash".to_string(), "Glob".to_string()]),
        ..Default::default()
    })
    .await
    .unwrap();

    match agent.prompt("List the files in the current directory. Be brief.").await {
        Ok(result) => {
            println!("\n{}", result.text);
            println!(
                "\n({} turns, {} tokens)",
                result.num_turns,
                result.usage.input_tokens + result.usage.output_tokens
            );
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    agent.close().await;
}
