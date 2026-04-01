// Example 11: Web Chat UI
//
// A web-based chat interface for interacting with the agent.
// Shows streaming text, tool calls with input/output, and cost tracking.
//
// Run: cargo run --example web-chat
// Then open http://localhost:8082

use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use open_agent_sdk::types;
use serde::Serialize;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const INDEX_HTML: &str = include_str!("index.html");

#[derive(Serialize)]
struct SseEvent {
    #[serde(rename = "type")]
    event_type: String,
    data: serde_json::Value,
}

struct AppState {
    agent: Option<Agent>,
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8082".to_string());
    let state = Arc::new(Mutex::new(AppState { agent: None }));

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("Failed to bind");

    println!("🚀 Web Chat running at http://localhost:{}", port);

    loop {
        let (stream, _) = listener.accept().await.expect("Failed to accept");
        let state = state.clone();

        tokio::spawn(async move {
            handle_connection(stream, state).await;
        });
    }
}

async fn handle_connection(mut stream: tokio::net::TcpStream, state: Arc<Mutex<AppState>>) {
    let mut buf = vec![0u8; 8192];
    let n = match stream.read(&mut buf).await {
        Ok(n) => n,
        Err(_) => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    let first_line = request.lines().next().unwrap_or("");

    if first_line.starts_with("GET / ") || first_line.starts_with("GET /index.html") {
        // Serve index.html
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            INDEX_HTML.len(),
            INDEX_HTML
        );
        let _ = stream.write_all(response.as_bytes()).await;
    } else if first_line.starts_with("POST /api/new") {
        // New session
        {
            let mut state = state.lock().await;
            if let Some(agent) = state.agent.take() {
                agent.close().await;
            }
        }
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\"}";
        let _ = stream.write_all(response.as_bytes()).await;
    } else if first_line.starts_with("POST /api/chat") {
        // Extract body from request
        let body = extract_body(&request);
        let message = extract_json_field(&body, "message");

        if message.is_empty() {
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\n\r\ninvalid request";
            let _ = stream.write_all(response.as_bytes()).await;
            return;
        }

        // SSE headers
        let headers = "HTTP/1.1 200 OK\r\n\
                       Content-Type: text/event-stream\r\n\
                       Cache-Control: no-cache\r\n\
                       Connection: keep-alive\r\n\
                       X-Accel-Buffering: no\r\n\r\n";
        let _ = stream.write_all(headers.as_bytes()).await;

        let start = std::time::Instant::now();

        // Get or create agent
        let agent = {
            let mut state = state.lock().await;
            if state.agent.is_none() {
                let model = std::env::var("CODEANY_MODEL").unwrap_or_else(|_| "sonnet-4-6".to_string());
                match Agent::new(AgentOptions {
                    model: Some(model),
                    max_turns: Some(20),
                    ..Default::default()
                })
                .await
                {
                    Ok(a) => state.agent = Some(a),
                    Err(e) => {
                        let event = format_sse("error", &serde_json::json!({"message": e}));
                        let _ = stream.write_all(event.as_bytes()).await;
                        let done = format_sse("done", &serde_json::json!(null));
                        let _ = stream.write_all(done.as_bytes()).await;
                        return;
                    }
                }
            }
            // We need a mutable reference; use a different approach
            state.agent.take()
        };

        let mut agent = agent.unwrap();

        let (mut rx, handle) = agent.query(&message).await;

        while let Some(event) = rx.recv().await {
            let sse = match &event {
                SDKMessage::Assistant { message, .. } => {
                    let mut parts = Vec::new();
                    for block in &message.content {
                        match block {
                            types::ContentBlock::Text { text } if !text.is_empty() => {
                                parts.push(format_sse(
                                    "text",
                                    &serde_json::json!({"text": text}),
                                ));
                            }
                            types::ContentBlock::ToolUse { id, name, input } => {
                                parts.push(format_sse(
                                    "tool_use",
                                    &serde_json::json!({"id": id, "name": name, "input": input}),
                                ));
                            }
                            types::ContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                                parts.push(format_sse(
                                    "thinking",
                                    &serde_json::json!({"thinking": thinking}),
                                ));
                            }
                            _ => {}
                        }
                    }
                    parts.join("")
                }
                SDKMessage::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } => format_sse(
                    "tool_result",
                    &serde_json::json!({
                        "tool_use_id": tool_use_id,
                        "content": content,
                        "is_error": is_error,
                    }),
                ),
                SDKMessage::Result {
                    usage,
                    num_turns,
                    cost_usd,
                    ..
                } => format_sse(
                    "result",
                    &serde_json::json!({
                        "num_turns": num_turns,
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "cost": cost_usd,
                        "duration_ms": start.elapsed().as_millis() as u64,
                    }),
                ),
                SDKMessage::Error { message } => {
                    format_sse("error", &serde_json::json!({"message": message}))
                }
                _ => String::new(),
            };

            if !sse.is_empty() {
                if stream.write_all(sse.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stream.flush().await;
            }
        }

        let _ = handle.await;

        let done = format_sse("done", &serde_json::json!(null));
        let _ = stream.write_all(done.as_bytes()).await;

        // Put agent back
        let mut state = state.lock().await;
        state.agent = Some(agent);
    } else {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\nNot Found";
        let _ = stream.write_all(response.as_bytes()).await;
    }
}

fn format_sse(event_type: &str, data: &serde_json::Value) -> String {
    let payload = serde_json::json!({"type": event_type, "data": data});
    format!("data: {}\n\n", serde_json::to_string(&payload).unwrap())
}

fn extract_body(request: &str) -> String {
    if let Some(idx) = request.find("\r\n\r\n") {
        request[idx + 4..].to_string()
    } else {
        String::new()
    }
}

fn extract_json_field(body: &str, field: &str) -> String {
    // Simple JSON field extraction without a full parser dependency
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        value
            .get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    }
}

use tokio::io::AsyncReadExt;
