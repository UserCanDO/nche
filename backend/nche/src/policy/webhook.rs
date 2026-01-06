//! Webhook-based policy evaluation.
//!
//! When a tenant has `policy_mode = "webhook"`, policy evaluation is delegated
//! to the tenant's policy webhook endpoint. This allows tenants to implement
//! custom policy logic.
//!
//! ## Request Format
//! ```json
//! {
//!     "tool": "payment_charge",
//!     "params": { "amount_cents": 50000 },
//!     "session": {
//!         "id": "sess_xxx",
//!         "autonomy_level": "supervised",
//!         "actor_id": "user_123",
//!         "actor_type": "user"
//!     },
//!     "agent": {
//!         "id": "agt_xxx",
//!         "name": "Support Bot"
//!     }
//! }
//! ```
//!
//! ## Response Format
//! ```json
//! {
//!     "decision": "allow" | "deny" | "require_approval",
//!     "reason": "Amount exceeds $100 limit"
//! }
//! ```
//!
//! ## Error Handling
//! - Timeout: Falls back to `require_approval`
//! - HTTP error: Falls back to `require_approval`
//! - Invalid response: Falls back to `require_approval`

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use time::OffsetDateTime;

use crate::domain::{Action, Agent, PolicyResult, Session, Tenant};

type HmacSha256 = Hmac<Sha256>;

/// Request sent to the tenant's policy webhook
#[derive(Debug, Serialize)]
pub struct PolicyWebhookRequest {
    pub tool: String,
    pub params: serde_json::Value,
    pub session: SessionInfo,
    pub agent: Option<AgentInfo>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub autonomy_level: String,
    pub actor_id: String,
    pub actor_type: String,
}

#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
}

/// Response expected from the tenant's policy webhook
#[derive(Debug, Deserialize)]
pub struct PolicyWebhookResponse {
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Result of webhook policy evaluation
pub struct WebhookPolicyResult {
    pub result: PolicyResult,
    pub reason: String,
}

/// Client for making policy webhook calls
pub struct PolicyWebhookClient {
    http_client: reqwest::Client,
}

impl PolicyWebhookClient {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    /// Evaluate policy via webhook.
    ///
    /// Returns None if the tenant doesn't have webhook policy mode configured.
    /// Returns Some with the policy decision if webhook mode is enabled.
    /// Falls back to require_approval on any error.
    pub async fn evaluate(
        &self,
        tenant: &Tenant,
        session: &Session,
        action: &Action,
        agent: Option<&Agent>,
    ) -> Option<WebhookPolicyResult> {
        // Check if tenant has webhook policy mode
        let policy_mode = tenant.policy_mode.as_deref().unwrap_or("builtin");
        if policy_mode != "webhook" {
            return None;
        }

        // Must have a webhook URL
        let webhook_url = match &tenant.policy_webhook_url {
            Some(url) => url,
            None => {
                tracing::warn!(
                    tenant_id = %tenant.id,
                    "Tenant has policy_mode=webhook but no policy_webhook_url"
                );
                return Some(WebhookPolicyResult {
                    result: PolicyResult::RequireApproval,
                    reason: "Policy webhook URL not configured".to_string(),
                });
            }
        };

        // Build the request
        let request = PolicyWebhookRequest {
            tool: action.tool.clone(),
            params: action.params.clone(),
            session: SessionInfo {
                id: session.id.to_string(),
                autonomy_level: format!("{:?}", session.autonomy_level).to_lowercase(),
                actor_id: session.actor_id.clone(),
                actor_type: format!("{:?}", session.actor_type).to_lowercase(),
            },
            agent: agent.map(|a| AgentInfo {
                id: a.id.to_string(),
                name: a.name.clone(),
            }),
        };

        // Make the webhook call
        match self.call_webhook(tenant, webhook_url, &request).await {
            Ok(response) => Some(self.parse_response(response)),
            Err(e) => {
                tracing::warn!(
                    tenant_id = %tenant.id,
                    error = %e,
                    "Policy webhook call failed, falling back to require_approval"
                );
                Some(WebhookPolicyResult {
                    result: PolicyResult::RequireApproval,
                    reason: format!("Policy webhook error: {}", e),
                })
            }
        }
    }

    async fn call_webhook(
        &self,
        tenant: &Tenant,
        webhook_url: &str,
        request: &PolicyWebhookRequest,
    ) -> Result<PolicyWebhookResponse, String> {
        let timeout_ms = tenant.policy_webhook_timeout_ms.unwrap_or(500) as u64;
        let timestamp = OffsetDateTime::now_utc().unix_timestamp();

        let body = serde_json::to_string(request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        // Build request with optional signature
        let mut http_request = self
            .http_client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Nche-Timestamp", timestamp.to_string())
            .timeout(Duration::from_millis(timeout_ms));

        // Sign if secret is configured
        if let Some(secret) = &tenant.policy_webhook_secret {
            let signature = self.generate_signature(secret, timestamp, &body)?;
            http_request = http_request.header("X-Nche-Signature", signature);
        }

        let response = http_request
            .body(body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP {} from policy webhook", response.status()));
        }

        let response_body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        serde_json::from_str(&response_body)
            .map_err(|e| format!("Failed to parse response: {} (body: {})", e, response_body))
    }

    fn parse_response(&self, response: PolicyWebhookResponse) -> WebhookPolicyResult {
        let result = match response.decision.to_lowercase().as_str() {
            "allow" => PolicyResult::Allow,
            "deny" => PolicyResult::Deny,
            "require_approval" => PolicyResult::RequireApproval,
            other => {
                tracing::warn!(
                    decision = other,
                    "Unknown decision from policy webhook, treating as require_approval"
                );
                PolicyResult::RequireApproval
            }
        };

        WebhookPolicyResult {
            result,
            reason: response
                .reason
                .unwrap_or_else(|| format!("Policy webhook returned: {}", response.decision)),
        }
    }

    fn generate_signature(&self, secret: &str, timestamp: i64, body: &str) -> Result<String, String> {
        let message = format!("{}.{}", timestamp, body);

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| format!("HMAC error: {}", e))?;

        mac.update(message.as_bytes());
        let result = mac.finalize();

        Ok(hex::encode(result.into_bytes()))
    }
}

impl Default for PolicyWebhookClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ActionId, ActionState, AgentId, ActorType, AutonomyLevel, SessionId, TenantId};
    use time::OffsetDateTime;

    fn make_tenant_webhook() -> Tenant {
        let now = OffsetDateTime::now_utc();
        Tenant {
            id: TenantId::from_string("ten_test".to_string()),
            name: "Test Tenant".to_string(),
            webhook_url: None,
            webhook_secret: None,
            webhook_events: None,
            internal_domains: None,
            execution_webhook_url: None,
            execution_webhook_secret: None,
            execution_webhook_timeout_ms: None,
            policy_mode: Some("webhook".to_string()),
            policy_webhook_url: Some("https://example.com/policy".to_string()),
            policy_webhook_secret: Some("secret".to_string()),
            policy_webhook_timeout_ms: Some(500),
            created_at: now,
            updated_at: now,
        }
    }

    fn make_tenant_builtin() -> Tenant {
        let now = OffsetDateTime::now_utc();
        Tenant {
            id: TenantId::from_string("ten_test".to_string()),
            name: "Test Tenant".to_string(),
            webhook_url: None,
            webhook_secret: None,
            webhook_events: None,
            internal_domains: None,
            execution_webhook_url: None,
            execution_webhook_secret: None,
            execution_webhook_timeout_ms: None,
            policy_mode: Some("builtin".to_string()),
            policy_webhook_url: None,
            policy_webhook_secret: None,
            policy_webhook_timeout_ms: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_session() -> Session {
        Session {
            id: SessionId::from_string("sess_test".to_string()),
            tenant_id: TenantId::from_string("ten_test".to_string()),
            agent_id: AgentId::from_string("agt_test".to_string()),
            actor_id: "user_123".to_string(),
            actor_type: ActorType::User,
            autonomy_level: AutonomyLevel::Supervised,
            created_at: OffsetDateTime::now_utc(),
            ended_at: None,
        }
    }

    fn make_action() -> Action {
        Action {
            id: ActionId::from_string("act_test".to_string()),
            tenant_id: TenantId::from_string("ten_test".to_string()),
            session_id: SessionId::from_string("sess_test".to_string()),
            tool: "payment_charge".to_string(),
            params: serde_json::json!({"amount_cents": 5000}),
            state: ActionState::Proposed,
            policy_result: None,
            policy_reason: None,
            result: None,
            error: None,
            execution_result: None,
            executed_by: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[tokio::test]
    async fn test_builtin_mode_returns_none() {
        let client = PolicyWebhookClient::new();
        let tenant = make_tenant_builtin();
        let session = make_session();
        let action = make_action();

        let result = client.evaluate(&tenant, &session, &action, None).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_webhook_mode_without_url() {
        let client = PolicyWebhookClient::new();
        let mut tenant = make_tenant_webhook();
        tenant.policy_webhook_url = None;
        let session = make_session();
        let action = make_action();

        let result = client.evaluate(&tenant, &session, &action, None).await;
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.result, PolicyResult::RequireApproval);
        assert!(r.reason.contains("not configured"));
    }

    #[test]
    fn test_parse_response_allow() {
        let client = PolicyWebhookClient::new();
        let response = PolicyWebhookResponse {
            decision: "allow".to_string(),
            reason: Some("Test reason".to_string()),
        };
        let result = client.parse_response(response);
        assert_eq!(result.result, PolicyResult::Allow);
        assert_eq!(result.reason, "Test reason");
    }

    #[test]
    fn test_parse_response_deny() {
        let client = PolicyWebhookClient::new();
        let response = PolicyWebhookResponse {
            decision: "deny".to_string(),
            reason: Some("Blocked".to_string()),
        };
        let result = client.parse_response(response);
        assert_eq!(result.result, PolicyResult::Deny);
    }

    #[test]
    fn test_parse_response_require_approval() {
        let client = PolicyWebhookClient::new();
        let response = PolicyWebhookResponse {
            decision: "require_approval".to_string(),
            reason: None,
        };
        let result = client.parse_response(response);
        assert_eq!(result.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_parse_response_unknown_becomes_require_approval() {
        let client = PolicyWebhookClient::new();
        let response = PolicyWebhookResponse {
            decision: "unknown_value".to_string(),
            reason: None,
        };
        let result = client.parse_response(response);
        assert_eq!(result.result, PolicyResult::RequireApproval);
    }

    #[test]
    fn test_generate_signature() {
        let client = PolicyWebhookClient::new();
        let signature = client.generate_signature("secret", 1234567890, r#"{"test": true}"#).unwrap();
        assert_eq!(signature.len(), 64); // SHA256 hex is 64 chars
    }
}
