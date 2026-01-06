//! NCHE Rust Client
//!
//! A simple client for interacting with the NCHE Agent Control Plane API.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub id: String,
    pub tenant_id: String,
    pub agent_id: String,
    pub autonomy_level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub id: String,
    pub state: String,
    pub tool: String,
    pub params: Value,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    actor_id: String,
    actor_type: String,
    autonomy_level: String,
}

#[derive(Debug, Serialize)]
struct ProposeActionRequest {
    session_id: String,
    tool: String,
    params: Value,
}

pub struct NcheClient {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    session_id: Option<String>,
}

impl NcheClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
            session_id: None,
        }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }

    pub async fn create_session(
        &self,
        actor_id: &str,
        actor_type: &str,
        autonomy_level: &str,
    ) -> Result<Session> {
        let request = CreateSessionRequest {
            actor_id: actor_id.to_string(),
            actor_type: actor_type.to_string(),
            autonomy_level: autonomy_level.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/v1/sessions", self.base_url))
            .headers(self.headers())
            .json(&request)
            .send()
            .await
            .context("Failed to create session")?;

        if !response.status().is_success() {
            let error = response.text().await?;
            anyhow::bail!("Failed to create session: {}", error);
        }

        response.json().await.context("Failed to parse session response")
    }

    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/v1/sessions/{}", self.base_url, session_id))
            .headers(self.headers())
            .send()
            .await
            .context("Failed to end session")?;

        if !response.status().is_success() {
            let error = response.text().await?;
            anyhow::bail!("Failed to end session: {}", error);
        }

        Ok(())
    }

    pub async fn propose_action(&self, session_id: &str, tool: &str, params: Value) -> Result<Action> {
        let request = ProposeActionRequest {
            session_id: session_id.to_string(),
            tool: tool.to_string(),
            params,
        };

        let response = self
            .client
            .post(format!("{}/v1/actions", self.base_url))
            .headers(self.headers())
            .json(&request)
            .send()
            .await
            .context("Failed to propose action")?;

        if !response.status().is_success() {
            let error = response.text().await?;
            anyhow::bail!("Failed to propose action: {}", error);
        }

        response.json().await.context("Failed to parse action response")
    }

    pub async fn get_action(&self, action_id: &str) -> Result<Action> {
        let response = self
            .client
            .get(format!("{}/v1/actions/{}", self.base_url, action_id))
            .headers(self.headers())
            .send()
            .await
            .context("Failed to get action")?;

        if !response.status().is_success() {
            let error = response.text().await?;
            anyhow::bail!("Failed to get action: {}", error);
        }

        response.json().await.context("Failed to parse action response")
    }

    pub async fn wait_for_action(&self, action_id: &str, timeout: Duration) -> Result<Action> {
        let terminal_states = ["executed", "failed", "denied"];
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Action {} timed out after {:?}", action_id, timeout);
            }

            let action = self.get_action(action_id).await?;
            if terminal_states.contains(&action.state.as_str()) {
                return Ok(action);
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn execute_tool(&self, tool: &str, params: Value, timeout: Duration) -> Result<Action> {
        let session_id = self.session_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        let action = self.propose_action(session_id, tool, params).await?;
        self.wait_for_action(&action.id, timeout).await
    }
}

// Allow NcheClient to store session_id
impl NcheClient {
    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = Some(session_id);
    }
}
