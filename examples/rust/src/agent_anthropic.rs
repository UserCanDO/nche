//! NCHE Agent Example - Anthropic Claude
//!
//! This example demonstrates how to build an AI agent using Anthropic's Claude
//! that executes actions through NCHE for human oversight.
//!
//! # Usage
//! ```bash
//! export ANTHROPIC_API_KEY=your_api_key
//! export NCHE_API_KEY=your_nche_api_key
//! cargo run --bin agent_anthropic
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

mod nche_client;
use nche_client::NcheClient;

// Anthropic API types
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    tools: Vec<Tool>,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Tool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: MessageContent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: Value },
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: String,
}

fn get_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "send_email".to_string(),
            description: "Send an email to a recipient.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "Email address" },
                    "subject": { "type": "string", "description": "Subject line" },
                    "body": { "type": "string", "description": "Email body" }
                },
                "required": ["to", "subject", "body"]
            }),
        },
        Tool {
            name: "http_request".to_string(),
            description: "Make an HTTP request.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "method": { "type": "string", "enum": ["GET", "POST", "PUT", "DELETE"] },
                    "url": { "type": "string", "description": "URL to request" },
                    "headers": { "type": "object" },
                    "body": { "type": "string" }
                },
                "required": ["method", "url"]
            }),
        },
    ]
}

async fn call_anthropic(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[Message],
) -> Result<AnthropicResponse> {
    let request = AnthropicRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 4096,
        system: "You are a helpful assistant that can send emails and make HTTP requests. \
                 All actions are reviewed by humans before execution.".to_string(),
        tools: get_tools(),
        messages: messages.to_vec(),
    };

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to call Anthropic API")?;

    if !response.status().is_success() {
        let error = response.text().await?;
        anyhow::bail!("Anthropic API error: {}", error);
    }

    response.json().await.context("Failed to parse response")
}

async fn execute_tool_via_nche(
    nche: &NcheClient,
    session_id: &str,
    tool_name: &str,
    tool_input: Value,
) -> Result<String> {
    println!("\n[NCHE] Proposing action: {}", tool_name);
    println!("[NCHE] Parameters: {}", serde_json::to_string_pretty(&tool_input)?);

    let action = nche.propose_action(session_id, tool_name, tool_input).await?;
    let action = nche.wait_for_action(&action.id, Duration::from_secs(120)).await?;

    println!("[NCHE] Action state: {}", action.state);

    Ok(match action.state.as_str() {
        "executed" => serde_json::to_string(&action.result.unwrap_or(json!({"status": "success"})))?,
        "denied" => json!({"error": "Action was denied by human reviewer"}).to_string(),
        "failed" => json!({"error": action.error.unwrap_or_else(|| "Action failed".to_string())}).to_string(),
        state => json!({"error": format!("Unexpected state: {}", state)}).to_string(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let nche_url = env::var("NCHE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let nche_api_key = env::var("NCHE_API_KEY").context("NCHE_API_KEY is required")?;
    let anthropic_api_key = env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is required")?;

    let http_client = reqwest::Client::new();
    let nche = NcheClient::new(&nche_url, &nche_api_key);

    // Create NCHE session
    println!("[NCHE] Creating session...");
    let session = nche.create_session("claude-agent", "agent", "supervised").await?;
    println!("[NCHE] Session created: {}", session.id);

    let user_message = "Please send an email to team@example.com with subject 'Weekly Update' \
                        and body 'Hi team, here is this week's progress update.'";

    println!("\n[User] {}\n", user_message);

    let mut messages = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(user_message.to_string()),
    }];

    // Agent loop
    loop {
        let response = call_anthropic(&http_client, &anthropic_api_key, &messages).await?;

        // Check for text response
        for block in &response.content {
            if let ContentBlock::Text { text } = block {
                println!("[Claude] {}", text);
            }
        }

        // Check if done
        if response.stop_reason == "end_turn" {
            break;
        }

        // Extract tool uses
        let tool_uses: Vec<_> = response.content.iter()
            .filter_map(|b| {
                if let ContentBlock::ToolUse { id, name, input } = b {
                    Some((id.clone(), name.clone(), input.clone()))
                } else {
                    None
                }
            })
            .collect();

        if tool_uses.is_empty() {
            break;
        }

        // Add assistant message
        messages.push(Message {
            role: "assistant".to_string(),
            content: MessageContent::Blocks(response.content),
        });

        // Execute tools and collect results
        let mut tool_results = Vec::new();
        for (id, name, input) in tool_uses {
            let result = execute_tool_via_nche(&nche, &session.id, &name, input).await?;
            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: id,
                content: result,
            });
        }

        // Add tool results
        messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::Blocks(tool_results),
        });
    }

    // End session
    println!("\n[NCHE] Ending session...");
    nche.end_session(&session.id).await?;
    println!("[NCHE] Session ended");

    Ok(())
}
