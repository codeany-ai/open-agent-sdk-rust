# Open Agent SDK (Rust)

A lightweight, open-source Rust SDK for building AI agents. Run the full agent loop in-process — no CLI or subprocess required. Deploy anywhere: cloud, serverless, Docker, CI/CD.

Also available in [TypeScript](https://github.com/codeany-ai/open-agent-sdk-typescript) and [Go](https://github.com/codeany-ai/open-agent-sdk-go).

## Features

- **Agent Loop** — Streaming agentic loop with tool execution, multi-turn conversations, and cost tracking
- **Built-in Tools** — Bash, Read, Write, Edit, Glob, Grep, WebFetch, WebSearch, Agent (subagents), AskUser, TaskTools, ToolSearch
- **MCP Support** — Connect to MCP servers via stdio, HTTP, and SSE transports
- **Permission System** — Configurable tool approval with allow/deny rules and filesystem path validation
- **Hook System** — Pre/post tool-use hooks, post-sampling hooks, structured output enforcement
- **Extended Thinking** — Support for extended thinking with budget tokens
- **Cost Tracking** — Per-model token usage, API/tool duration, code change stats
- **Custom Tools** — Implement the `Tool` trait to add your own tools

## Quick Start

```toml
[dependencies]
open-agent-sdk = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

```rust
use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;

#[tokio::main]
async fn main() {
    let mut agent = Agent::new(AgentOptions::default()).await.unwrap();

    // Streaming
    let (mut rx, handle) = agent.query("What files are in this directory?").await;
    while let Some(event) = rx.recv().await {
        match event {
            SDKMessage::Assistant { message, .. } => {
                print!("{}", types::extract_text(&message));
            }
            SDKMessage::Result { text, cost_usd, .. } => {
                println!("\nDone (${:.4})", cost_usd);
            }
            _ => {}
        }
    }
    handle.await.unwrap();

    // Or use the blocking API
    let result = agent.prompt("Count lines in Cargo.toml").await.unwrap();
    println!("{}", result.text);

    agent.close().await;
}
```

## Examples

| #   | Example                                                         | Description                                      |
| --- | --------------------------------------------------------------- | ------------------------------------------------ |
| 01  | [Simple Query](examples/01-simple-query.rs)                     | Streaming query with tool calls                  |
| 02  | [Multi-Tool](examples/02-multi-tool.rs)                         | Glob + Bash multi-tool orchestration             |
| 03  | [Multi-Turn](examples/03-multi-turn.rs)                         | Multi-turn conversation with session persistence |
| 04  | [Prompt API](examples/04-prompt-api.rs)                         | Blocking `prompt()` for one-shot queries         |
| 05  | [Custom System Prompt](examples/05-custom-system-prompt.rs)     | Custom system prompt for code review             |
| 06  | [MCP Server](examples/06-mcp-server.rs)                         | MCP server integration (stdio transport)         |
| 07  | [Custom Tools](examples/07-custom-tools.rs)                     | Define and use custom tools                      |
| 08  | [One-shot Query](examples/08-oneshot-query.rs)                  | Quick one-shot agent query                       |
| 09  | [Subagents](examples/09-subagents.rs)                           | Specialized subagent with restricted tools       |
| 10  | [Permissions](examples/10-permissions.rs)                       | Read-only agent with allowed tools               |
| 11  | [Web Chat](examples/web/)                                       | Web-based chat UI with streaming                 |

Run any example:

```bash
export CODEANY_BASE_URL=https://openrouter.ai/api
export CODEANY_API_KEY=your-api-key
export CODEANY_MODEL=anthropic/claude-sonnet-4
cargo run --example 01-simple-query
```

For the web chat UI:

```bash
cargo run --example web-chat
# Open http://localhost:8082
```

## Custom Tools

Implement the `Tool` trait:

```rust
use async_trait::async_trait;
use open_agent_sdk::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "MyTool" }
    fn description(&self) -> &str { "Does something useful" }
    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "input".to_string(),
                json!({"type": "string", "description": "The input"}),
            )]),
            required: vec!["input".to_string()],
            additional_properties: Some(false),
        }
    }
    fn is_read_only(&self, _: &Value) -> bool { true }
    async fn call(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        Ok(ToolResult::text("result"))
    }
}

// Use it
let agent = Agent::new(AgentOptions {
    custom_tools: vec![Arc::new(MyTool)],
    ..Default::default()
}).await.unwrap();
```

## MCP Servers

```rust
use open_agent_sdk::types::McpServerConfig;

let agent = Agent::new(AgentOptions {
    mcp_servers: HashMap::from([(
        "filesystem".to_string(),
        McpServerConfig::Stdio {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), "/tmp".to_string()],
            env: HashMap::new(),
        },
    )]),
    ..Default::default()
}).await.unwrap();
```

## Architecture

```
open-agent-sdk-rust/
├── src/
│   ├── agent/          # Agent loop, query engine, options
│   ├── api/            # Messages API client (streaming + non-streaming)
│   ├── types/          # Core types: Message, Tool, ContentBlock, MCP
│   ├── tools/          # Built-in tool implementations + registry + executor
│   ├── mcp/            # MCP client (stdio, HTTP, SSE) + tool wrapping
│   ├── permissions/    # Permission rules, filesystem validation
│   ├── hooks/          # Pre/post tool-use hooks
│   ├── costtracker/    # Token usage and cost tracking
│   ├── context/        # System/user context injection (git status, AGENT.md)
│   └── utils/          # Token estimation, retry, messages, compaction
├── tests/              # Comprehensive test suite
└── examples/           # 11 runnable examples + web chat UI
```

## Configuration

Environment variables:

| Variable                     | Description                                  |
| ---------------------------- | -------------------------------------------- |
| `CODEANY_API_KEY`            | API key (required)                           |
| `CODEANY_MODEL`              | Default model (default: `sonnet-4-6`)        |
| `CODEANY_BASE_URL`           | API base URL override                        |
| `CODEANY_CUSTOM_HEADERS`     | Custom headers (comma-separated `key:value`) |
| `API_TIMEOUT_MS`             | API request timeout in ms                    |
| `HTTPS_PROXY` / `HTTP_PROXY` | Proxy URL                                    |

## Links

- Website: [codeany.ai](https://codeany.ai)
- TypeScript SDK: [github.com/codeany-ai/open-agent-sdk-typescript](https://github.com/codeany-ai/open-agent-sdk-typescript)
- Go SDK: [github.com/codeany-ai/open-agent-sdk-go](https://github.com/codeany-ai/open-agent-sdk-go)
- Issues: [github.com/codeany-ai/open-agent-sdk-rust/issues](https://github.com/codeany-ai/open-agent-sdk-rust/issues)

## License

MIT
