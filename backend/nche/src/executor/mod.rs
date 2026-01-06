//! Execution webhook dispatcher.
//!
//! Polls for actions in ReadyToExecute state and dispatches them to tenants
//! via webhook. Tenants execute the tools and report back via the result API.
//!
//! Flow:
//! 1. Poll for ready_to_execute actions
//! 2. Lock them (transition to pending_execution)
//! 3. Send execution webhook to tenant with action details
//! 4. Tenant executes the tool in their environment
//! 5. Tenant reports result via POST /v1/actions/:id/result

use std::sync::Arc;
use std::time::Duration;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::OffsetDateTime;
use tokio::time::interval;

use crate::db::Database;
use crate::domain::{Action, Tenant, WebhookEventType};
use crate::error::{NcheError, Result};

type HmacSha256 = Hmac<Sha256>;

/// Configuration for the execution webhook dispatcher
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// How often to poll for ready actions (in milliseconds)
    pub poll_interval_ms: u64,
    /// Maximum number of actions to lock per poll
    pub batch_size: i64,
    /// Whether to run the dispatcher
    pub enabled: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            batch_size: 10,
            enabled: true,
        }
    }
}

/// Background service that dispatches ready actions to tenants via webhook
pub struct ExecutionDispatcher {
    db: Arc<Database>,
    http_client: reqwest::Client,
    config: ExecutorConfig,
}

impl ExecutionDispatcher {
    pub fn new(db: Arc<Database>, config: ExecutorConfig) -> Self {
        Self {
            db,
            http_client: reqwest::Client::new(),
            config,
        }
    }

    /// Run the dispatcher loop (blocking)
    pub async fn run(&self) {
        if !self.config.enabled {
            tracing::info!("Execution dispatcher is disabled");
            return;
        }

        tracing::info!(
            "Starting execution dispatcher (poll interval: {}ms, batch size: {})",
            self.config.poll_interval_ms,
            self.config.batch_size
        );

        let mut interval = interval(Duration::from_millis(self.config.poll_interval_ms));

        loop {
            interval.tick().await;

            if let Err(e) = self.poll_and_dispatch().await {
                tracing::error!("Execution dispatcher poll failed: {}", e);
            }
        }
    }

    /// Single poll iteration - lock and dispatch ready actions
    async fn poll_and_dispatch(&self) -> Result<()> {
        // Atomically lock ready actions (transitions to pending_execution)
        let actions = self.db.lock_ready_actions(self.config.batch_size).await?;

        if actions.is_empty() {
            return Ok(());
        }

        tracing::debug!("Locked {} actions for execution dispatch", actions.len());

        // Dispatch each action concurrently
        let futures: Vec<_> = actions
            .into_iter()
            .map(|action| self.dispatch_action(action))
            .collect();

        futures::future::join_all(futures).await;

        Ok(())
    }

    /// Dispatch a single action to the tenant for execution
    async fn dispatch_action(&self, action: Action) {
        let action_id = action.id.clone();
        let tenant_id = action.tenant_id.clone();
        let session_id = action.session_id.clone();
        let tool = action.tool.clone();

        tracing::info!(
            action_id = %action_id,
            tool = %tool,
            "Dispatching action for execution"
        );

        // Get tenant to find execution webhook URL
        let tenant = match self.db.get_tenant(&tenant_id).await {
            Ok(Some(t)) => t,
            Ok(None) => {
                tracing::error!(
                    action_id = %action_id,
                    tenant_id = %tenant_id,
                    "Tenant not found, cannot dispatch"
                );
                // Mark as failed since we can't dispatch
                let _ = self
                    .db
                    .record_execution_result(
                        &tenant_id,
                        &action_id,
                        false,
                        None,
                        Some("Tenant not found"),
                        "system",
                    )
                    .await;
                return;
            }
            Err(e) => {
                tracing::error!(
                    action_id = %action_id,
                    "Failed to fetch tenant: {}",
                    e
                );
                return;
            }
        };

        // Check if tenant has execution webhook configured
        let webhook_url = match &tenant.execution_webhook_url {
            Some(url) => url.clone(),
            None => {
                tracing::warn!(
                    action_id = %action_id,
                    tenant_id = %tenant_id,
                    "Tenant has no execution webhook configured"
                );
                // Mark as failed - tenant must configure execution webhook
                let _ = self
                    .db
                    .record_execution_result(
                        &tenant_id,
                        &action_id,
                        false,
                        None,
                        Some("Tenant has no execution webhook configured"),
                        "system",
                    )
                    .await;
                return;
            }
        };

        // Send execution webhook
        let result = self.send_execution_webhook(&tenant, &webhook_url, &action).await;

        match result {
            Ok(()) => {
                tracing::info!(
                    action_id = %action_id,
                    "Execution webhook dispatched successfully"
                );

                // Log event
                if let Err(e) = self
                    .db
                    .create_event(
                        &tenant_id,
                        Some(&session_id),
                        Some(&action_id),
                        "action.dispatched",
                        serde_json::json!({
                            "tool": tool,
                            "webhook_url": webhook_url,
                        }),
                    )
                    .await
                {
                    tracing::error!(
                        action_id = %action_id,
                        "Failed to log dispatch event: {}",
                        e
                    );
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!(
                    action_id = %action_id,
                    error = %error_msg,
                    "Failed to dispatch execution webhook"
                );

                // Mark action as failed since we couldn't deliver
                let _ = self
                    .db
                    .record_execution_result(
                        &tenant_id,
                        &action_id,
                        false,
                        None,
                        Some(&error_msg),
                        "system",
                    )
                    .await;

                // Queue failure notification webhook
                let _ = crate::webhooks::queue_webhook(
                    &self.db,
                    &tenant_id,
                    WebhookEventType::ActionFailed,
                    serde_json::json!({
                        "action_id": action_id.to_string(),
                        "session_id": session_id.to_string(),
                        "tool": tool,
                        "error": error_msg,
                    }),
                )
                .await;
            }
        }
    }

    /// Send the execution webhook to the tenant
    async fn send_execution_webhook(
        &self,
        tenant: &Tenant,
        webhook_url: &str,
        action: &Action,
    ) -> Result<()> {
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();
        let timeout_ms = tenant.execution_webhook_timeout_ms.unwrap_or(30000) as u64;

        // Build the execution request payload
        let payload = serde_json::json!({
            "event_type": "execute_action",
            "timestamp": timestamp,
            "action": {
                "id": action.id.to_string(),
                "session_id": action.session_id.to_string(),
                "tool": action.tool,
                "params": action.params,
            }
        });

        let body_str = serde_json::to_string(&payload)
            .map_err(|e| NcheError::Internal(e.to_string()))?;

        // Generate signature if secret is configured
        let signature = if let Some(secret) = &tenant.execution_webhook_secret {
            Some(self.generate_signature(secret, timestamp, &body_str)?)
        } else {
            None
        };

        let mut request = self
            .http_client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Nche-Timestamp", timestamp.to_string())
            .header("X-Nche-Action-Id", action.id.to_string())
            .timeout(Duration::from_millis(timeout_ms));

        if let Some(sig) = signature {
            request = request.header("X-Nche-Signature", sig);
        }

        let response = request.body(body_str).send().await.map_err(|e| {
            NcheError::WebhookDelivery {
                message: format!("Execution webhook request failed: {}", e),
            }
        })?;

        if !response.status().is_success() {
            return Err(NcheError::WebhookDelivery {
                message: format!("Execution webhook returned HTTP {}", response.status()),
            });
        }

        Ok(())
    }

    fn generate_signature(&self, secret: &str, timestamp: i64, body: &str) -> Result<String> {
        let message = format!("{}.{}", timestamp, body);

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| NcheError::Internal(format!("HMAC error: {}", e)))?;

        mac.update(message.as_bytes());
        let result = mac.finalize();

        Ok(hex::encode(result.into_bytes()))
    }
}

/// Spawn the execution dispatcher as a background task
pub fn spawn_executor(db: Arc<Database>, config: ExecutorConfig) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let dispatcher = ExecutionDispatcher::new(db, config);
        dispatcher.run().await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ExecutorConfig::default();
        assert_eq!(config.poll_interval_ms, 1000);
        assert_eq!(config.batch_size, 10);
        assert!(config.enabled);
    }

    #[test]
    fn test_custom_config() {
        let config = ExecutorConfig {
            poll_interval_ms: 500,
            batch_size: 20,
            enabled: false,
        };
        assert_eq!(config.poll_interval_ms, 500);
        assert_eq!(config.batch_size, 20);
        assert!(!config.enabled);
    }
}
