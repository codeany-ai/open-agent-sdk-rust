use crate::types::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Create a registry with all default built-in tools.
    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        // File I/O tools
        registry.register(Arc::new(super::bash::BashTool));
        registry.register(Arc::new(super::fileread::FileReadTool));
        registry.register(Arc::new(super::filewrite::FileWriteTool));
        registry.register(Arc::new(super::fileedit::FileEditTool));
        registry.register(Arc::new(super::glob_tool::GlobTool));
        registry.register(Arc::new(super::grep::GrepTool));

        // Web tools
        registry.register(Arc::new(super::webfetch::WebFetchTool));
        registry.register(Arc::new(super::websearch::WebSearchTool::default()));

        // User interaction
        registry.register(Arc::new(super::askuser::AskUserTool::default()));

        // Task tools
        let task_store = super::tasks::TaskStore::new();
        registry.register(Arc::new(super::tasks::TaskCreateTool::new(
            task_store.clone(),
        )));
        registry.register(Arc::new(super::tasks::TaskGetTool::new(
            task_store.clone(),
        )));
        registry.register(Arc::new(super::tasks::TaskListTool::new(
            task_store.clone(),
        )));
        registry.register(Arc::new(super::tasks::TaskUpdateTool::new(
            task_store.clone(),
        )));
        registry.register(Arc::new(super::tasks::TaskStopTool::new(
            task_store.clone(),
        )));
        registry.register(Arc::new(super::tasks::TaskOutputTool::new(task_store)));

        // Tool search
        registry.register(Arc::new(super::toolsearch::ToolSearchTool::default()));

        // Notebook edit
        registry.register(Arc::new(super::notebook_edit::NotebookEditTool));

        // Inter-agent messaging
        let mailbox = super::send_message::new_mailbox();
        registry.register(Arc::new(super::send_message::SendMessageTool::new(mailbox)));

        // Team tools
        let team_store = super::team_tools::TeamStore::new();
        registry.register(Arc::new(super::team_tools::TeamCreateTool::new(
            team_store.clone(),
        )));
        registry.register(Arc::new(super::team_tools::TeamDeleteTool::new(team_store)));

        // Plan mode tools
        let plan_state = super::plan_tools::PlanState::new();
        registry.register(Arc::new(super::plan_tools::EnterPlanModeTool::new(
            plan_state.clone(),
        )));
        registry.register(Arc::new(super::plan_tools::ExitPlanModeTool::new(plan_state)));

        // Worktree tools
        let worktree_store = super::worktree_tools::new_worktree_store();
        registry.register(Arc::new(super::worktree_tools::EnterWorktreeTool::new(
            worktree_store.clone(),
        )));
        registry.register(Arc::new(super::worktree_tools::ExitWorktreeTool::new(
            worktree_store,
        )));

        // Todo tool
        let todo_store = super::todo_tool::TodoStore::new();
        registry.register(Arc::new(super::todo_tool::TodoWriteTool::new(todo_store)));

        // Cron tools
        let cron_store = super::cron_tools::CronStore::new();
        registry.register(Arc::new(super::cron_tools::CronCreateTool::new(
            cron_store.clone(),
        )));
        registry.register(Arc::new(super::cron_tools::CronDeleteTool::new(
            cron_store.clone(),
        )));
        registry.register(Arc::new(super::cron_tools::CronListTool::new(cron_store)));

        // Config tool
        let config_store = super::config_tool::ConfigStore::new();
        registry.register(Arc::new(super::config_tool::ConfigTool::new(config_store)));

        // LSP tool
        registry.register(Arc::new(super::lsp_tool::LSPTool));

        // MCP resource tools
        let mcp_client: Arc<RwLock<Option<Arc<crate::mcp::McpClient>>>> =
            Arc::new(RwLock::new(None));
        registry.register(Arc::new(
            super::mcp_resource_tools::ListMcpResourcesTool::new(mcp_client.clone()),
        ));
        registry.register(Arc::new(
            super::mcp_resource_tools::ReadMcpResourceTool::new(mcp_client),
        ));

        registry
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn filter<F>(&self, predicate: F) -> Vec<Arc<dyn Tool>>
    where
        F: Fn(&dyn Tool) -> bool,
    {
        self.tools
            .values()
            .filter(|t| predicate(t.as_ref()))
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Remove tools by name.
    pub fn remove(&mut self, names: &[&str]) {
        for name in names {
            self.tools.remove(*name);
        }
    }

    /// Keep only the specified tools.
    pub fn retain(&mut self, names: &[&str]) {
        let name_set: std::collections::HashSet<&str> = names.iter().copied().collect();
        self.tools.retain(|k, _| name_set.contains(k.as_str()));
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}
