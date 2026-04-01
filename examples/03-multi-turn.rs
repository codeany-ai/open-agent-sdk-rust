// Example 3: Multi-Turn Conversation
//
// Three turns with context retention: create a file, read it back,
// then delete it.
//
// Run: cargo run --example 03-multi-turn

use open_agent_sdk::{Agent, AgentOptions};

#[tokio::main]
async fn main() {
    println!("--- Example 3: Multi-Turn ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(10),
        ..Default::default()
    })
    .await
    .unwrap();

    // Turn 1: Create a file
    println!("\n[Turn 1] Creating file...");
    match agent
        .prompt("Use Bash to create a file /tmp/sdk-test-rust.txt with the text 'Hello from Rust SDK!'")
        .await
    {
        Ok(result) => println!("Response: {}", result.text),
        Err(e) => eprintln!("Error: {}", e),
    }

    // Turn 2: Read it back
    println!("\n[Turn 2] Reading file...");
    match agent
        .prompt("Now read /tmp/sdk-test-rust.txt and tell me what it says")
        .await
    {
        Ok(result) => println!("Response: {}", result.text),
        Err(e) => eprintln!("Error: {}", e),
    }

    // Turn 3: Delete it
    println!("\n[Turn 3] Cleaning up...");
    match agent
        .prompt("Delete /tmp/sdk-test-rust.txt using Bash")
        .await
    {
        Ok(result) => {
            println!("Response: {}", result.text);
            println!("\nTotal messages: {}", agent.get_messages().len());
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    agent.close().await;
}
