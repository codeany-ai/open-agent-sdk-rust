use open_agent_sdk::hooks::*;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_hook_config_default() {
    let config = HookConfig::default();
    assert!(config.pre_tool_use.is_empty());
    assert!(config.post_tool_use.is_empty());
    assert!(config.post_sampling.is_empty());
    assert!(config.stop.is_empty());
}

#[tokio::test]
async fn test_pre_tool_use_hook_allow() {
    let config = HookConfig {
        pre_tool_use: vec![HookRule {
            matcher: "*".to_string(),
            handler: Arc::new(|_input| {
                Box::pin(async move {
                    HookOutput {
                        blocked: false,
                        message: None,
                        ..Default::default()
                    }
                })
            }),
        }],
        ..Default::default()
    };

    let result = config.run_pre_tool_use("Bash", &json!({})).await;
    assert!(result.is_none()); // Not blocked
}

#[tokio::test]
async fn test_pre_tool_use_hook_block() {
    let config = HookConfig {
        pre_tool_use: vec![HookRule {
            matcher: "Bash".to_string(),
            handler: Arc::new(|_input| {
                Box::pin(async move {
                    HookOutput {
                        blocked: true,
                        message: Some("Bash is blocked".to_string()),
                        ..Default::default()
                    }
                })
            }),
        }],
        ..Default::default()
    };

    let result = config.run_pre_tool_use("Bash", &json!({})).await;
    assert!(result.is_some());
    assert!(result.unwrap().blocked);

    // Different tool should not be blocked
    let result = config.run_pre_tool_use("Read", &json!({})).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_hook_matcher_wildcard() {
    let config = HookConfig {
        pre_tool_use: vec![HookRule {
            matcher: "mcp__*".to_string(),
            handler: Arc::new(|_input| {
                Box::pin(async move {
                    HookOutput {
                        blocked: true,
                        message: Some("MCP blocked".to_string()),
                        ..Default::default()
                    }
                })
            }),
        }],
        ..Default::default()
    };

    let result = config
        .run_pre_tool_use("mcp__server__tool", &json!({}))
        .await;
    assert!(result.is_some());

    let result = config.run_pre_tool_use("Read", &json!({})).await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_hook_matcher_pipe_or() {
    let config = HookConfig {
        pre_tool_use: vec![HookRule {
            matcher: "Bash|Edit|Write".to_string(),
            handler: Arc::new(|_input| {
                Box::pin(async move {
                    HookOutput {
                        blocked: true,
                        message: None,
                        ..Default::default()
                    }
                })
            }),
        }],
        ..Default::default()
    };

    assert!(config
        .run_pre_tool_use("Bash", &json!({}))
        .await
        .is_some());
    assert!(config
        .run_pre_tool_use("Edit", &json!({}))
        .await
        .is_some());
    assert!(config
        .run_pre_tool_use("Write", &json!({}))
        .await
        .is_some());
    assert!(config
        .run_pre_tool_use("Read", &json!({}))
        .await
        .is_none());
}

#[tokio::test]
async fn test_post_tool_use_hook() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    let config = HookConfig {
        post_tool_use: vec![HookRule {
            matcher: "Bash".to_string(),
            handler: Arc::new(move |_input| {
                let called = called_clone.clone();
                Box::pin(async move {
                    called.store(true, Ordering::SeqCst);
                    HookOutput::default()
                })
            }),
        }],
        ..Default::default()
    };

    config
        .run_post_tool_use("Bash", &json!({}), "output")
        .await;
    assert!(called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_hook_input_fields() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let verified = Arc::new(AtomicBool::new(false));
    let verified_clone = verified.clone();

    let config = HookConfig {
        pre_tool_use: vec![HookRule {
            matcher: "*".to_string(),
            handler: Arc::new(move |input| {
                let verified = verified_clone.clone();
                Box::pin(async move {
                    assert_eq!(input.event, HookEvent::PreToolUse);
                    assert_eq!(input.tool_name.as_deref(), Some("Bash"));
                    assert!(input.tool_input.is_some());
                    verified.store(true, Ordering::SeqCst);
                    HookOutput::default()
                })
            }),
        }],
        ..Default::default()
    };

    config
        .run_pre_tool_use("Bash", &json!({"command": "ls"}))
        .await;
    assert!(verified.load(Ordering::SeqCst));
}
