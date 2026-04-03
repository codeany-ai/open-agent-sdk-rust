use serde_json::Value;
use std::sync::Arc;

/// Hook event types.
#[derive(Debug, Clone, PartialEq)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PostSampling,
    SessionStart,
    SessionEnd,
    Stop,
    SubagentStart,
    SubagentStop,
    UserPromptSubmit,
    PermissionRequest,
    PermissionDenied,
    TaskCreated,
    TaskCompleted,
    ConfigChange,
    CwdChanged,
    FileChanged,
    Notification,
    PreCompact,
    PostCompact,
    TeammateIdle,
}

/// Input passed to a hook handler.
#[derive(Debug, Clone)]
pub struct HookInput {
    pub event: HookEvent,
    pub tool_name: Option<String>,
    pub tool_input: Option<Value>,
    pub tool_output: Option<String>,
    pub tool_use_id: Option<String>,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub error: Option<String>,
}

/// Output from a hook handler.
#[derive(Debug, Clone, Default)]
pub struct HookOutput {
    /// If true, block the tool execution.
    pub blocked: bool,
    /// Optional message to return instead.
    pub message: Option<String>,
    /// Permission update (tool name + behavior).
    pub permission_update: Option<PermissionUpdate>,
    /// Notification to emit.
    pub notification: Option<HookNotification>,
}

/// Permission update returned by a hook.
#[derive(Debug, Clone)]
pub struct PermissionUpdate {
    pub tool: String,
    pub behavior: PermissionBehavior,
}

/// Permission behavior: allow or deny.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionBehavior {
    Allow,
    Deny,
}

/// Notification emitted by a hook.
#[derive(Debug, Clone)]
pub struct HookNotification {
    pub title: String,
    pub body: String,
    pub level: Option<NotificationLevel>,
}

/// Notification severity level.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

/// Hook function type.
pub type HookFn =
    Arc<dyn Fn(HookInput) -> futures::future::BoxFuture<'static, HookOutput> + Send + Sync>;

/// A hook rule with a matcher pattern and handler.
pub struct HookRule {
    pub matcher: String,
    pub handler: HookFn,
}

/// Hook configuration for the agent.
pub struct HookConfig {
    pub pre_tool_use: Vec<HookRule>,
    pub post_tool_use: Vec<HookRule>,
    pub post_tool_use_failure: Vec<HookRule>,
    pub post_sampling: Vec<HookRule>,
    pub session_start: Vec<HookRule>,
    pub session_end: Vec<HookRule>,
    pub stop: Vec<HookRule>,
    pub subagent_start: Vec<HookRule>,
    pub subagent_stop: Vec<HookRule>,
    pub user_prompt_submit: Vec<HookRule>,
    pub permission_request: Vec<HookRule>,
    pub permission_denied: Vec<HookRule>,
    pub task_created: Vec<HookRule>,
    pub task_completed: Vec<HookRule>,
    pub config_change: Vec<HookRule>,
    pub cwd_changed: Vec<HookRule>,
    pub file_changed: Vec<HookRule>,
    pub notification: Vec<HookRule>,
    pub pre_compact: Vec<HookRule>,
    pub post_compact: Vec<HookRule>,
    pub teammate_idle: Vec<HookRule>,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            pre_tool_use: Vec::new(),
            post_tool_use: Vec::new(),
            post_tool_use_failure: Vec::new(),
            post_sampling: Vec::new(),
            session_start: Vec::new(),
            session_end: Vec::new(),
            stop: Vec::new(),
            subagent_start: Vec::new(),
            subagent_stop: Vec::new(),
            user_prompt_submit: Vec::new(),
            permission_request: Vec::new(),
            permission_denied: Vec::new(),
            task_created: Vec::new(),
            task_completed: Vec::new(),
            config_change: Vec::new(),
            cwd_changed: Vec::new(),
            file_changed: Vec::new(),
            notification: Vec::new(),
            pre_compact: Vec::new(),
            post_compact: Vec::new(),
            teammate_idle: Vec::new(),
        }
    }
}

impl HookConfig {
    /// Get the hook rules for a given event.
    pub fn rules_for_event(&self, event: &HookEvent) -> &[HookRule] {
        match event {
            HookEvent::PreToolUse => &self.pre_tool_use,
            HookEvent::PostToolUse => &self.post_tool_use,
            HookEvent::PostToolUseFailure => &self.post_tool_use_failure,
            HookEvent::PostSampling => &self.post_sampling,
            HookEvent::SessionStart => &self.session_start,
            HookEvent::SessionEnd => &self.session_end,
            HookEvent::Stop => &self.stop,
            HookEvent::SubagentStart => &self.subagent_start,
            HookEvent::SubagentStop => &self.subagent_stop,
            HookEvent::UserPromptSubmit => &self.user_prompt_submit,
            HookEvent::PermissionRequest => &self.permission_request,
            HookEvent::PermissionDenied => &self.permission_denied,
            HookEvent::TaskCreated => &self.task_created,
            HookEvent::TaskCompleted => &self.task_completed,
            HookEvent::ConfigChange => &self.config_change,
            HookEvent::CwdChanged => &self.cwd_changed,
            HookEvent::FileChanged => &self.file_changed,
            HookEvent::Notification => &self.notification,
            HookEvent::PreCompact => &self.pre_compact,
            HookEvent::PostCompact => &self.post_compact,
            HookEvent::TeammateIdle => &self.teammate_idle,
        }
    }

    /// Run hooks for a given event with tool context.
    pub async fn run_event(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
        tool_input: Option<&Value>,
        tool_output: Option<&str>,
    ) -> Vec<HookOutput> {
        let rules = self.rules_for_event(&event);
        let mut outputs = Vec::new();
        for rule in rules {
            if let Some(name) = tool_name {
                if !matches_tool(&rule.matcher, name) {
                    continue;
                }
            }
            let input = HookInput {
                event: event.clone(),
                tool_name: tool_name.map(|s| s.to_string()),
                tool_input: tool_input.cloned(),
                tool_output: tool_output.map(|s| s.to_string()),
                tool_use_id: None,
                session_id: None,
                cwd: None,
                error: None,
            };
            let output = (rule.handler)(input).await;
            outputs.push(output);
        }
        outputs
    }

    /// Run pre-tool-use hooks for a given tool name.
    pub async fn run_pre_tool_use(
        &self,
        tool_name: &str,
        tool_input: &Value,
    ) -> Option<HookOutput> {
        for rule in &self.pre_tool_use {
            if matches_tool(&rule.matcher, tool_name) {
                let input = HookInput {
                    event: HookEvent::PreToolUse,
                    tool_name: Some(tool_name.to_string()),
                    tool_input: Some(tool_input.clone()),
                    tool_output: None,
                    tool_use_id: None,
                    session_id: None,
                    cwd: None,
                    error: None,
                };
                let output = (rule.handler)(input).await;
                if output.blocked {
                    return Some(output);
                }
            }
        }
        None
    }

    /// Run post-tool-use hooks for a given tool name.
    pub async fn run_post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &Value,
        tool_output: &str,
    ) {
        for rule in &self.post_tool_use {
            if matches_tool(&rule.matcher, tool_name) {
                let input = HookInput {
                    event: HookEvent::PostToolUse,
                    tool_name: Some(tool_name.to_string()),
                    tool_input: Some(tool_input.clone()),
                    tool_output: Some(tool_output.to_string()),
                    tool_use_id: None,
                    session_id: None,
                    cwd: None,
                    error: None,
                };
                (rule.handler)(input).await;
            }
        }
    }

    /// Run stop hooks.
    pub async fn run_stop(&self) {
        for rule in &self.stop {
            let input = HookInput {
                event: HookEvent::Stop,
                tool_name: None,
                tool_input: None,
                tool_output: None,
                tool_use_id: None,
                session_id: None,
                cwd: None,
                error: None,
            };
            (rule.handler)(input).await;
        }
    }
}

/// Check if a tool name matches a hook matcher pattern.
fn matches_tool(matcher: &str, tool_name: &str) -> bool {
    if matcher == "*" || matcher.is_empty() {
        return true;
    }

    // Support pipe-separated patterns: "Bash|Edit|Write"
    if matcher.contains('|') {
        return matcher.split('|').any(|p| matches_tool(p.trim(), tool_name));
    }

    // Support prefix matching: "mcp__*"
    if matcher.ends_with('*') {
        let prefix = &matcher[..matcher.len() - 1];
        return tool_name.starts_with(prefix);
    }

    // Exact match
    matcher == tool_name
}
