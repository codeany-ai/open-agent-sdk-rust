// Example 7: Custom Tools
//
// Shows how to define and use custom tools alongside built-in tools.
//
// Run: cargo run --example 07-custom-tools

use async_trait::async_trait;
use open_agent_sdk::types::{self, Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};
use open_agent_sdk::{Agent, AgentOptions, SDKMessage};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// --- GetWeather tool ---

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "GetWeather"
    }
    fn description(&self) -> &str {
        "Get current weather for a city. Returns temperature and conditions."
    }
    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "city".to_string(),
                json!({"type": "string", "description": "City name (e.g., \"Tokyo\", \"London\")"}),
            )]),
            required: vec!["city".to_string()],
            additional_properties: Some(false),
        }
    }
    fn is_read_only(&self, _: &Value) -> bool {
        true
    }
    async fn call(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let city = input
            .get("city")
            .and_then(|c| c.as_str())
            .unwrap_or("Unknown");
        let temps: HashMap<&str, i32> = HashMap::from([
            ("tokyo", 22),
            ("london", 14),
            ("beijing", 25),
            ("new york", 18),
            ("paris", 16),
        ]);
        let temp = temps
            .get(city.to_lowercase().as_str())
            .copied()
            .unwrap_or(20);
        Ok(ToolResult::text(format!(
            "Weather in {}: {}°C, partly cloudy",
            city, temp
        )))
    }
}

// --- Calculator tool ---

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "Calculator"
    }
    fn description(&self) -> &str {
        "Evaluate a simple mathematical expression with two numbers and an operator (+, -, *, /, **)."
    }
    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([(
                "expression".to_string(),
                json!({"type": "string", "description": "Math expression (e.g., \"2 ** 10\", \"42 * 17\")"}),
            )]),
            required: vec!["expression".to_string()],
            additional_properties: Some(false),
        }
    }
    fn is_read_only(&self, _: &Value) -> bool {
        true
    }
    async fn call(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let expr = input
            .get("expression")
            .and_then(|e| e.as_str())
            .unwrap_or("0");
        let result = eval_simple(expr);
        Ok(ToolResult::text(match result {
            Ok(val) => format!("{} = {}", expr, val),
            Err(e) => format!("Error: {}", e),
        }))
    }
}

fn eval_simple(expr: &str) -> Result<String, String> {
    for op in &["**", "*", "/", "+", "-"] {
        if let Some(idx) = expr.find(op) {
            if idx == 0 {
                continue;
            }
            let left: f64 = expr[..idx]
                .trim()
                .parse()
                .map_err(|_| "invalid number".to_string())?;
            let right: f64 = expr[idx + op.len()..]
                .trim()
                .parse()
                .map_err(|_| "invalid number".to_string())?;
            let result = match *op {
                "**" => left.powf(right),
                "*" => left * right,
                "/" => {
                    if right == 0.0 {
                        return Err("division by zero".to_string());
                    }
                    left / right
                }
                "+" => left + right,
                "-" => left - right,
                _ => return Err(format!("unknown operator: {}", op)),
            };
            return if result == (result as i64) as f64 {
                Ok(format!("{}", result as i64))
            } else {
                Ok(format!("{:.4}", result))
            };
        }
    }
    Err(format!("cannot evaluate: {}", expr))
}

#[tokio::main]
async fn main() {
    println!("--- Example 7: Custom Tools ---");

    let mut agent = Agent::new(AgentOptions {
        max_turns: Some(10),
        custom_tools: vec![Arc::new(WeatherTool), Arc::new(CalculatorTool)],
        ..Default::default()
    })
    .await
    .unwrap();

    println!("Loaded tools with 2 custom tools (GetWeather, Calculator)\n");

    let (mut rx, handle) = agent
        .query("What is the weather in Tokyo and London? Also calculate 2 ** 10 * 3. Be brief.")
        .await;

    while let Some(event) = rx.recv().await {
        match event {
            SDKMessage::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        types::ContentBlock::ToolUse { name, input, .. } => {
                            println!("[{}] {}", name, serde_json::to_string(input).unwrap_or_default());
                        }
                        types::ContentBlock::Text { text } => {
                            if !text.trim().is_empty() {
                                println!("\n{}", text);
                            }
                        }
                        _ => {}
                    }
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
