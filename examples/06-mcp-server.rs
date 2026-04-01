// Example 6: MCP Server Integration
//
// Connects to the filesystem MCP server via stdio transport.
// Requires: npm install -g @modelcontextprotocol/server-filesystem
//
// Run: cargo run --example 06-mcp-server

use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types::{self, McpServerConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    println!("--- Example 6: MCP Server ---");

    let mut mcp_servers = HashMap::new();
    mcp_servers.insert(
        "filesystem".to_string(),
        McpServerConfig::Stdio {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                "/tmp".to_string(),
            ],
            env: HashMap::new(),
        },
    );

    let agent = Agent::new(AgentOptions {
        mcp_servers,
        max_turns: Some(10),
        ..Default::default()
    })
    .await;

    match agent {
        Ok(mut agent) => {
            let (mut rx, handle) = agent
                .query("List files in /tmp using the filesystem MCP tools. Be brief.")
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
                        println!("\n--- Done ---");
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
        Err(e) => {
            eprintln!(
                "Failed to create agent: {}\n\
                 Make sure the MCP server is installed:\n\
                 npm install -g @modelcontextprotocol/server-filesystem",
                e
            );
        }
    }
}
