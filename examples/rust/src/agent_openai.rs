//! NCHE Agent Example - OpenAI GPT
//!
//! This example demonstrates how to build an AI agent using OpenAI's GPT
//! that executes actions through NCHE for human oversight.
//!
//! # Usage
//! ```bash
//! export OPENAI_API_KEY=your_api_key
//! export NCHE_API_KEY=your_nche_api_key
//! cargo run --bin agent_openai
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

mod nche_client;
use nche_client::NcheClient;

// OpenAI API types
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    tools: Vec<OpenAITool>,
    tool_choice: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Serialize)]
struct OpenAIFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: OpenAIMessage,
}

fn get_tools() -> Vec<OpenAITool> {
    vec![
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunction {
                name: "send_email".to_string(),
                description: "Send an email to a recipient.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "to": { "type": "string", "description": "Email address" },
                        "subject": { "type": "string", "description": "Subject line" },
                        "body": { "type": "string", "description": "Email body" }
                    },
                    "required": ["to", "subject", "body"]
                }),
            },
        },
        OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunction {
                name: "http_request".to_string(),
                description: "Make an HTTP request.".to_string(),
                parameters: json!({
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
        },
    ]
}

async fn call_openai(
    client: &reqwest::Client,
    api_key: &str,
    messages: &[OpenAIMessage],
) -> Result<OpenAIResponse> {
    let request = OpenAIRequest {
        model: "gpt-4o".to_string(),
        messages: messages.to_vec(),
        tools: get_tools(),
        tool_choice: "auto".to_string(),
    };

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to call OpenAI API")?;

    if !response.status().is_success() {
        let error = response.text().await?;
        anyhow::bail!("OpenAI API error: {}", error);
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
    let openai_api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY is required")?;

    let http_client = reqwest::Client::new();
    let nche = NcheClient::new(&nche_url, &nche_api_key);

    // Create NCHE session
    println!("[NCHE] Creating session...");
    let session = nche.create_session("gpt-agent", "agent", "supervised").await?;
    println!("[NCHE] Session created: {}", session.id);

    let user_message = "Please send an email to team@example.com with subject 'Weekly Update' \
                        and body 'Hi team, here is this week's progress update.'";

    println!("\n[User] {}\n", user_message);

    let mut messages = vec![
        OpenAIMessage {
            role: "system".to_string(),
            content: Some("You are a helpful assistant that can send emails and make HTTP requests. \
                          All actions are reviewed by humans before execution.".to_string()),
            tool_calls: None,
            tool_call_id: None,
        },
        OpenAIMessage {
            role: "user".to_string(),
            content: Some(user_message.to_string()),
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // Agent loop
    loop {
        let response = call_openai(&http_client, &openai_api_key, &messages).await?;
        let message = &response.choices[0].message;

        // Print any text content
        if let Some(content) = &message.content {
            if !content.is_empty() {
                println!("[GPT] {}", content);
            }
        }

        // Check if there are tool calls
        let tool_calls = message.tool_calls.clone().unwrap_or_default();
        if tool_calls.is_empty() {
            break;
        }

        // Add assistant message
        messages.push(message.clone());

        // Execute tools and add results
        for tool_call in tool_calls {
            let tool_input: Value = serde_json::from_str(&tool_call.function.arguments)
                .context("Failed to parse tool arguments")?;

            let result = execute_tool_via_nche(
                &nche,
                &session.id,
                &tool_call.function.name,
                tool_input,
            ).await?;

            messages.push(OpenAIMessage {
                role: "tool".to_string(),
                content: Some(result),
                tool_calls: None,
                tool_call_id: Some(tool_call.id),
            });
        }
    }

    // End session
    println!("\n[NCHE] Ending session...");
    nche.end_session(&session.id).await?;
    println!("[NCHE] Session ended");

    Ok(())
}
