//! # Open Agent SDK
//!
//! A Rust framework for building autonomous AI agents that run the full
//! agentic loop in-process. Supports 15+ built-in tools, MCP integration,
//! permission systems, cost tracking, and more.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use open_agent_sdk::{Agent, AgentOptions};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut agent = Agent::new(AgentOptions::default()).await.unwrap();
//!     let result = agent.prompt("What files are in the current directory?").await.unwrap();
//!     println!("{}", result.text);
//! }
//! ```

pub mod agent;
pub mod api;
pub mod context;
pub mod costtracker;
pub mod hooks;
pub mod mcp;
pub mod permissions;
pub mod session;
pub mod tools;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use agent::{Agent, AgentOptions, SubagentDefinition};
pub use api::{ApiClient, ApiType, LLMProvider, ProviderResponse};
pub use costtracker::CostTracker;
pub use hooks::{
    HookConfig, HookEvent, HookFn, HookInput, HookNotification, HookOutput, HookRule,
    NotificationLevel, PermissionBehavior, PermissionUpdate,
};
pub use mcp::McpClient;
pub use tools::ToolRegistry;
pub use types::{
    ApiToolParam, CanUseToolFn, ContentBlock, Message, MessageRole, PermissionDecision,
    PermissionMode, QueryResult, SDKMessage, ThinkingConfig, Tool, ToolError, ToolInputSchema,
    SandboxFilesystemConfig, SandboxNetworkConfig, SandboxSettings, ToolResult, ToolResultContent,
    ToolUseContext, Usage,
};
pub use session::{
    append_to_session, delete_session, fork_session, get_session_info, get_session_messages,
    list_sessions, load_session, new_metadata, rename_session, save_session, tag_session,
    SessionData, SessionMetadata,
};
pub use utils::compact::{
    build_compaction_prompt, compact_conversation, strip_images_from_messages,
};
pub use utils::file_cache::FileStateCache;
pub use utils::tokens::{estimate_cost, estimate_tokens};
