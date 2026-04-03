//! # Session Storage & Management
//!
//! Persists conversation transcripts to disk for resumption.
//! Manages session lifecycle (create, resume, list, fork, delete, rename, tag).

use crate::types::Message;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Session metadata persisted alongside the transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub id: String,
    pub cwd: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Session data stored on disk (metadata + messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub metadata: SessionMetadata,
    pub messages: Vec<Message>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the root sessions directory: `~/.open-agent-sdk/sessions`.
fn get_sessions_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    Path::new(&home)
        .join(".open-agent-sdk")
        .join("sessions")
}

/// Return the directory for a specific session.
fn get_session_path(session_id: &str) -> PathBuf {
    get_sessions_dir().join(session_id)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Persist a session to `~/.open-agent-sdk/sessions/{id}/transcript.json`.
pub async fn save_session(
    session_id: &str,
    messages: &[Message],
    metadata: &SessionMetadata,
) -> Result<(), String> {
    let dir = get_session_path(session_id);
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create session directory: {e}"))?;

    let data = SessionData {
        metadata: SessionMetadata {
            id: session_id.to_string(),
            cwd: metadata.cwd.clone(),
            model: metadata.model.clone(),
            created_at: metadata.created_at.clone(),
            updated_at: Utc::now().to_rfc3339(),
            message_count: messages.len(),
            summary: metadata.summary.clone(),
            title: metadata.title.clone(),
            tags: metadata.tags.clone(),
        },
        messages: messages.to_vec(),
    };

    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize session: {e}"))?;

    fs::write(dir.join("transcript.json"), json)
        .await
        .map_err(|e| format!("Failed to write session file: {e}"))?;

    Ok(())
}

/// Load a session from disk. Returns `None` if the session does not exist or
/// the transcript cannot be parsed.
pub async fn load_session(session_id: &str) -> Result<Option<SessionData>, String> {
    let file_path = get_session_path(session_id).join("transcript.json");
    match fs::read_to_string(&file_path).await {
        Ok(content) => {
            let data: SessionData = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse session: {e}"))?;
            Ok(Some(data))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Failed to read session file: {e}")),
    }
}

/// List all sessions sorted by `updatedAt` descending (most recent first).
pub async fn list_sessions() -> Result<Vec<SessionMetadata>, String> {
    let dir = get_sessions_dir();
    let mut entries = match fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Failed to read sessions directory: {e}")),
    };

    let mut sessions: Vec<SessionMetadata> = Vec::new();

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| format!("Failed to read directory entry: {e}"))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(Some(data)) = load_session(&name).await {
            sessions.push(data.metadata);
        }
    }

    // Sort by updatedAt descending
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(sessions)
}

/// Fork a session — create a copy with a new ID. Returns the new session ID.
pub async fn fork_session(
    source_session_id: &str,
    new_session_id: Option<&str>,
) -> Result<Option<String>, String> {
    let data = match load_session(source_session_id).await? {
        Some(d) => d,
        None => return Ok(None),
    };

    let fork_id = new_session_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let now = Utc::now().to_rfc3339();
    let meta = SessionMetadata {
        id: fork_id.clone(),
        cwd: data.metadata.cwd,
        model: data.metadata.model,
        created_at: now,
        updated_at: String::new(), // will be overwritten by save_session
        message_count: data.messages.len(),
        summary: Some(format!("Forked from session {source_session_id}")),
        title: data.metadata.title,
        tags: data.metadata.tags,
    };

    save_session(&fork_id, &data.messages, &meta).await?;
    Ok(Some(fork_id))
}

/// Get messages from a session.
pub async fn get_session_messages(session_id: &str) -> Result<Vec<Message>, String> {
    match load_session(session_id).await? {
        Some(data) => Ok(data.messages),
        None => Ok(Vec::new()),
    }
}

/// Append a message to an existing session.
pub async fn append_to_session(session_id: &str, message: Message) -> Result<(), String> {
    let mut data = match load_session(session_id).await? {
        Some(d) => d,
        None => return Err(format!("Session '{session_id}' not found")),
    };

    data.messages.push(message);
    data.metadata.message_count = data.messages.len();

    save_session(session_id, &data.messages, &data.metadata).await
}

/// Delete a session directory from disk.
pub async fn delete_session(session_id: &str) -> Result<bool, String> {
    let path = get_session_path(session_id);
    match fs::remove_dir_all(&path).await {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("Failed to delete session: {e}")),
    }
}

/// Get metadata about a specific session.
pub async fn get_session_info(session_id: &str) -> Result<Option<SessionMetadata>, String> {
    match load_session(session_id).await? {
        Some(data) => Ok(Some(data.metadata)),
        None => Ok(None),
    }
}

/// Rename (update the title of) a session.
pub async fn rename_session(session_id: &str, title: &str) -> Result<(), String> {
    let data = match load_session(session_id).await? {
        Some(d) => d,
        None => return Err(format!("Session '{session_id}' not found")),
    };

    let meta = SessionMetadata {
        title: Some(title.to_string()),
        ..data.metadata
    };

    save_session(session_id, &data.messages, &meta).await
}

/// Add or update tags on a session. Pass `None` to clear tags.
pub async fn tag_session(session_id: &str, tags: Option<Vec<String>>) -> Result<(), String> {
    let data = match load_session(session_id).await? {
        Some(d) => d,
        None => return Err(format!("Session '{session_id}' not found")),
    };

    let meta = SessionMetadata {
        tags,
        ..data.metadata
    };

    save_session(session_id, &data.messages, &meta).await
}

/// Create a new `SessionMetadata` with sensible defaults.
pub fn new_metadata(id: &str, cwd: &str, model: &str) -> SessionMetadata {
    let now = Utc::now().to_rfc3339();
    SessionMetadata {
        id: id.to_string(),
        cwd: cwd.to_string(),
        model: model.to_string(),
        created_at: now.clone(),
        updated_at: now,
        message_count: 0,
        summary: None,
        title: None,
        tags: None,
    }
}
