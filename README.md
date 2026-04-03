# Open Agent SDK (Rust)

A lightweight, open-source Rust SDK for building AI agents. Run the full agent loop in-process — no CLI or subprocess required. Deploy anywhere: cloud, serverless, Docker, CI/CD.

Also available in [TypeScript](https://github.com/codeany-ai/open-agent-sdk-typescript) and [Go](https://github.com/codeany-ai/open-agent-sdk-go).

## Features

- **Multi-Provider** — Supports both Anthropic Messages API and OpenAI Chat Completions API, with auto-detection by model name
- **Agent Loop** — Streaming agentic loop with tool execution, multi-turn conversations, and cost tracking
- **25+ Built-in Tools** — Bash, Read, Write, Edit, Glob, Grep, WebFetch, WebSearch, Agent (subagents), AskUser, Tasks, Teams, Plans, Worktrees, Todos, Cron, LSP, Config, MCP Resources, ToolSearch
- **Session Persistence** — Save, load, fork, and resume conversations across sessions
- **MCP Support** — Connect to MCP servers via stdio, HTTP, and SSE transports
- **Permission System** — Configurable tool approval with allow/deny rules and filesystem path validation
- **Hook System** — 20+ hook events: PreToolUse, PostToolUse, SessionStart/End, SubagentStart/Stop, TaskCreated/Completed, FileChanged, PreCompact/PostCompact, and more
- **Extended Thinking** — Support for extended thinking with budget tokens
- **Cost Tracking** — Per-model token usage and pricing for Anthropic, OpenAI, and DeepSeek models
- **Custom Tools** — Implement the `Tool` trait to add your own tools
- **Auto-Compaction** — Multi-tier context compression (micro-compact, LLM summarization, image stripping)
- **Sandbox Config** — Network and filesystem restrictions for tool execution
- **File State Cache** — LRU cache for file state tracking and staleness detection

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

## Session Persistence

```rust
use open_agent_sdk::session::*;

// Save current session
save_session(&session_data).await.unwrap();

// List all sessions
let sessions = list_sessions().await.unwrap();

// Resume a session
let data = load_session("session-id").await.unwrap();

// Fork a session
let new_id = fork_session("session-id").await.unwrap();
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

## Built-in Tools

| Tool | Description | Read-Only |
|------|-------------|-----------|
| **Bash** | Execute shell commands | No |
| **Read** | Read files with line numbers | Yes |
| **Write** | Create/overwrite files | No |
| **Edit** | String replacement in files | No |
| **Glob** | File pattern matching | Yes |
| **Grep** | Regex search (ripgrep) | Yes |
| **NotebookEdit** | Edit Jupyter notebook cells | No |
| **WebFetch** | Fetch URL content | Yes |
| **WebSearch** | Web search (pluggable) | Yes |
| **AskUserQuestion** | Interactive user prompts | No |
| **SendMessage** | Inter-agent messaging | No |
| **TeamCreate/Delete** | Multi-agent team management | No |
| **EnterPlanMode/ExitPlanMode** | Structured planning mode | No |
| **EnterWorktree/ExitWorktree** | Git worktree isolation | No |
| **TodoWrite** | Session todo list | No |
| **TaskCreate/Get/List/Update/Stop/Output** | Task tracking (6 tools) | Mixed |
| **CronCreate/Delete/List** | Scheduled task management | Mixed |
| **Config** | Session configuration | Mixed |
| **LSP** | Language Server Protocol operations | Yes |
| **ListMcpResources/ReadMcpResource** | MCP resource access | Yes |
| **ToolSearch** | Discover available tools | Yes |

## Multi-Provider Support

The SDK automatically detects which API format to use based on the model name:

| Models | API Format | Auto-detected |
|--------|-----------|---------------|
| Claude (sonnet, opus, haiku) | Anthropic Messages | Yes |
| GPT-4o, O1, O3, O4 | OpenAI Chat Completions | Yes |
| DeepSeek, Qwen, Mistral | OpenAI Chat Completions | Yes |
| LLaMA, Gemma, Gemini, Yi, GLM | OpenAI Chat Completions | Yes |

Override with `CODEANY_API_TYPE=openai-completions` or `CODEANY_API_TYPE=anthropic-messages`.

Works with any OpenAI-compatible endpoint (OpenRouter, vLLM, Ollama, LiteLLM, etc.):

```bash
export CODEANY_BASE_URL=https://openrouter.ai/api
export CODEANY_API_KEY=your-key
export CODEANY_MODEL=anthropic/claude-sonnet-4
cargo run --example 01-simple-query
```

## Architecture

```
open-agent-sdk-rust/
├── src/
│   ├── agent/          # Agent loop, query engine, options
│   ├── api/            # Multi-provider API abstraction
│   │   ├── provider.rs # LLMProvider trait + auto-detection
│   │   ├── anthropic.rs# Anthropic Messages API (SSE streaming)
│   │   └── openai.rs   # OpenAI Chat Completions API (SSE streaming)
│   ├── types/          # Core types: Message, Tool, ContentBlock, MCP, Sandbox
│   ├── tools/          # 25+ built-in tool implementations + registry + executor
│   ├── mcp/            # MCP client (stdio, HTTP, SSE) + tool wrapping
│   ├── permissions/    # Permission rules, filesystem validation
│   ├── hooks/          # 20+ hook events with pre/post tool-use lifecycle
│   ├── costtracker/    # Token usage and cost tracking (Anthropic, OpenAI, DeepSeek)
│   ├── context/        # System/user context injection (git status, AGENT.md)
│   ├── session/        # Session persistence (save, load, fork, resume)
│   └── utils/          # Token estimation, retry, messages, compaction, file cache
├── tests/              # Comprehensive test suite (116 tests)
└── examples/           # 11 runnable examples + web chat UI
```

## Configuration

Environment variables:

| Variable                     | Description                                           |
| ---------------------------- | ----------------------------------------------------- |
| `CODEANY_API_KEY`            | API key (required)                                    |
| `CODEANY_MODEL`              | Default model (default: `sonnet-4-6`)                 |
| `CODEANY_BASE_URL`           | API base URL override                                 |
| `CODEANY_API_TYPE`           | Force API format: `anthropic-messages` or `openai-completions` |
| `CODEANY_CUSTOM_HEADERS`     | Custom headers (comma-separated `key:value`)          |
| `API_TIMEOUT_MS`             | API request timeout in ms                             |
| `HTTPS_PROXY` / `HTTP_PROXY` | Proxy URL                                             |

## Links

- Website: [codeany.ai](https://codeany.ai)
- TypeScript SDK: [github.com/codeany-ai/open-agent-sdk-typescript](https://github.com/codeany-ai/open-agent-sdk-typescript)
- Go SDK: [github.com/codeany-ai/open-agent-sdk-go](https://github.com/codeany-ai/open-agent-sdk-go)
- Issues: [github.com/codeany-ai/open-agent-sdk-rust/issues](https://github.com/codeany-ai/open-agent-sdk-rust/issues)

## License

MIT
