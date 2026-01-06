use std::sync::Arc;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::OffsetDateTime;
use tokio::time::interval;

use crate::db::Database;
use crate::domain::{Tenant, TenantId, WebhookDeliveryStatus, WebhookEventType};
use crate::error::{NcheError, Result};

type HmacSha256 = Hmac<Sha256>;

/// Configuration for the webhook dispatcher
#[derive(Debug, Clone)]
pub struct WebhookDispatcherConfig {
    /// How often to poll for pending webhooks (in milliseconds)
    pub poll_interval_ms: u64,
    /// Maximum number of webhooks to process per poll
    pub batch_size: i64,
    /// Maximum number of retry attempts
    pub max_retries: i32,
    /// Base delay for exponential backoff (in seconds)
    pub base_retry_delay_secs: i64,
    /// Whether the dispatcher is enabled
    pub enabled: bool,
}

impl Default for WebhookDispatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 5000,
            batch_size: 20,
            max_retries: 5,
            base_retry_delay_secs: 60,
            enabled: true,
        }
    }
}

pub struct WebhookSender {
    http_client: reqwest::Client,
}

impl WebhookSender {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn send(
        &self,
        tenant: &Tenant,
        event_type: WebhookEventType,
        payload: serde_json::Value,
    ) -> Result<()> {
        let webhook_url = tenant.webhook_url.as_ref().ok_or_else(|| NcheError::WebhookDelivery {
            message: "No webhook URL configured for tenant".to_string(),
        })?;

        // Check if tenant wants this event type
        if !self.should_send_event(tenant, event_type) {
            return Ok(());
        }

        let timestamp = OffsetDateTime::now_utc().unix_timestamp();
        let body = serde_json::json!({
            "event_type": event_type.to_string(),
            "timestamp": timestamp,
            "data": payload
        });

        let body_str = serde_json::to_string(&body).map_err(|e| NcheError::Internal(e.to_string()))?;

        // Generate signature if secret is configured
        let signature = if let Some(secret) = &tenant.webhook_secret {
            Some(self.generate_signature(secret, timestamp, &body_str)?)
        } else {
            None
        };

        let mut request = self
            .http_client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Nche-Timestamp", timestamp.to_string());

        if let Some(sig) = signature {
            request = request.header("X-Nche-Signature", sig);
        }

        let response = request.body(body_str).send().await.map_err(|e| {
            NcheError::WebhookDelivery {
                message: format!("Failed to send webhook: {}", e),
            }
        })?;

        if !response.status().is_success() {
            return Err(NcheError::WebhookDelivery {
                message: format!("Webhook returned status: {}", response.status()),
            });
        }

        Ok(())
    }

    fn should_send_event(&self, tenant: &Tenant, event_type: WebhookEventType) -> bool {
        match &tenant.webhook_events {
            Some(events) => {
                if let Some(arr) = events.as_array() {
                    arr.iter()
                        .any(|e| e.as_str() == Some(&event_type.to_string()))
                } else {
                    true // If not an array, send all events
                }
            }
            None => true, // If no filter, send all events
        }
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

impl Default for WebhookSender {
    fn default() -> Self {
        Self::new()
    }
}

/// Background service that dispatches pending webhooks with retry logic
pub struct WebhookDispatcher {
    db: Arc<Database>,
    sender: WebhookSender,
    config: WebhookDispatcherConfig,
}

impl WebhookDispatcher {
    pub fn new(db: Arc<Database>, config: WebhookDispatcherConfig) -> Self {
        Self {
            db,
            sender: WebhookSender::new(),
            config,
        }
    }

    /// Run the dispatcher loop (blocking)
    pub async fn run(&self) {
        if !self.config.enabled {
            tracing::info!("Webhook dispatcher is disabled");
            return;
        }

        tracing::info!(
            "Starting webhook dispatcher (poll interval: {}ms, batch size: {}, max retries: {})",
            self.config.poll_interval_ms,
            self.config.batch_size,
            self.config.max_retries
        );

        let mut interval = interval(Duration::from_millis(self.config.poll_interval_ms));

        loop {
            interval.tick().await;

            if let Err(e) = self.poll_and_dispatch().await {
                tracing::error!("Webhook dispatcher poll failed: {}", e);
            }
        }
    }

    /// Single poll iteration - fetch and dispatch pending webhooks
    async fn poll_and_dispatch(&self) -> Result<()> {
        let deliveries = self.db.get_ready_webhook_deliveries(self.config.batch_size).await?;

        if deliveries.is_empty() {
            return Ok(());
        }

        tracing::debug!("Processing {} pending webhook deliveries", deliveries.len());

        for delivery in deliveries {
            self.process_delivery(delivery).await;
        }

        Ok(())
    }

    /// Process a single webhook delivery
    async fn process_delivery(&self, delivery: crate::domain::WebhookDelivery) {
        let delivery_id = delivery.id.clone();
        let tenant_id = delivery.tenant_id.clone();
        let event_type = delivery.event_type.clone();

        tracing::debug!(
            delivery_id = %delivery_id,
            event_type = %event_type,
            attempts = delivery.attempts,
            "Processing webhook delivery"
        );

        // Get tenant for webhook URL and secret
        let tenant = match self.db.get_tenant(&tenant_id).await {
            Ok(Some(t)) => t,
            Ok(None) => {
                tracing::warn!(
                    delivery_id = %delivery_id,
                    tenant_id = %tenant_id,
                    "Tenant not found, marking delivery as failed"
                );
                let _ = self.db.mark_webhook_failed(&delivery_id).await;
                return;
            }
            Err(e) => {
                tracing::error!(
                    delivery_id = %delivery_id,
                    "Failed to fetch tenant: {}",
                    e
                );
                return;
            }
        };

        // Check if tenant has webhook configured
        if tenant.webhook_url.is_none() {
            tracing::debug!(
                delivery_id = %delivery_id,
                "Tenant has no webhook URL configured, marking as delivered"
            );
            let _ = self
                .db
                .update_webhook_delivery_status(&delivery_id, WebhookDeliveryStatus::Delivered, None)
                .await;
            return;
        }

        // Attempt delivery
        let start = Instant::now();
        let result = self.attempt_delivery(&tenant, &delivery).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(http_status) => {
                // Record successful attempt
                let _ = self
                    .db
                    .add_webhook_attempt(&delivery_id, Some(http_status), duration_ms, None)
                    .await;

                // Mark as delivered
                let _ = self
                    .db
                    .update_webhook_delivery_status(
                        &delivery_id,
                        WebhookDeliveryStatus::Delivered,
                        None,
                    )
                    .await;

                tracing::info!(
                    delivery_id = %delivery_id,
                    http_status = http_status,
                    duration_ms = duration_ms,
                    "Webhook delivered successfully"
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                let attempts = delivery.attempts + 1;

                // Record failed attempt
                let _ = self
                    .db
                    .add_webhook_attempt(&delivery_id, None, duration_ms, Some(&error_msg))
                    .await;

                if attempts >= self.config.max_retries {
                    // Max retries reached, mark as failed
                    let _ = self
                        .db
                        .update_webhook_delivery_status(
                            &delivery_id,
                            WebhookDeliveryStatus::Failed,
                            Some(&error_msg),
                        )
                        .await;

                    tracing::warn!(
                        delivery_id = %delivery_id,
                        attempts = attempts,
                        "Webhook delivery failed permanently after max retries"
                    );
                } else {
                    // Schedule retry with exponential backoff
                    let backoff_secs = self.config.base_retry_delay_secs * (2_i64.pow(attempts as u32 - 1));
                    let next_attempt = OffsetDateTime::now_utc() + time::Duration::seconds(backoff_secs);

                    let _ = self
                        .db
                        .update_webhook_next_attempt(&delivery_id, next_attempt)
                        .await;

                    tracing::debug!(
                        delivery_id = %delivery_id,
                        attempts = attempts,
                        next_attempt_in_secs = backoff_secs,
                        error = %error_msg,
                        "Webhook delivery failed, scheduled retry"
                    );
                }
            }
        }
    }

    /// Attempt to deliver a webhook, returns HTTP status code on success
    async fn attempt_delivery(
        &self,
        tenant: &Tenant,
        delivery: &crate::domain::WebhookDelivery,
    ) -> Result<u16> {
        let webhook_url = tenant.webhook_url.as_ref().ok_or_else(|| NcheError::WebhookDelivery {
            message: "No webhook URL configured".to_string(),
        })?;

        let timestamp = OffsetDateTime::now_utc().unix_timestamp();
        let body = serde_json::json!({
            "event_type": delivery.event_type,
            "timestamp": timestamp,
            "delivery_id": delivery.id.to_string(),
            "data": delivery.payload
        });

        let body_str = serde_json::to_string(&body)
            .map_err(|e| NcheError::Internal(e.to_string()))?;

        // Generate signature if secret is configured
        let signature = if let Some(secret) = &tenant.webhook_secret {
            Some(self.sender.generate_signature(secret, timestamp, &body_str)?)
        } else {
            None
        };

        let mut request = self
            .sender
            .http_client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Nche-Timestamp", timestamp.to_string())
            .header("X-Nche-Delivery-Id", delivery.id.to_string())
            .timeout(Duration::from_secs(30));

        if let Some(sig) = signature {
            request = request.header("X-Nche-Signature", sig);
        }

        let response = request.body(body_str).send().await.map_err(|e| {
            NcheError::WebhookDelivery {
                message: format!("HTTP request failed: {}", e),
            }
        })?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            return Err(NcheError::WebhookDelivery {
                message: format!("Webhook returned HTTP {}", status),
            });
        }

        Ok(status)
    }
}

/// Queue a webhook for delivery
pub async fn queue_webhook(
    db: &Database,
    tenant_id: &TenantId,
    event_type: WebhookEventType,
    payload: serde_json::Value,
) -> Result<()> {
    // Check if tenant has webhooks enabled
    let tenant = db.get_tenant(tenant_id).await?;
    let Some(tenant) = tenant else {
        return Ok(()); // No tenant, nothing to queue
    };

    if tenant.webhook_url.is_none() {
        return Ok(()); // No webhook URL configured
    }

    // Check if tenant wants this event type
    if let Some(events) = &tenant.webhook_events {
        if let Some(arr) = events.as_array() {
            let event_str = event_type.to_string();
            if !arr.iter().any(|e| e.as_str() == Some(&event_str)) {
                return Ok(()); // Tenant doesn't want this event type
            }
        }
    }

    // Create delivery record
    db.create_webhook_delivery(tenant_id, &event_type.to_string(), payload)
        .await?;

    tracing::debug!(
        tenant_id = %tenant_id,
        event_type = %event_type,
        "Queued webhook for delivery"
    );

    Ok(())
}

/// Spawn the webhook dispatcher as a background task
pub fn spawn_webhook_dispatcher(
    db: Arc<Database>,
    config: WebhookDispatcherConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let dispatcher = WebhookDispatcher::new(db, config);
        dispatcher.run().await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TenantId;

    // === WebhookDispatcherConfig Tests ===

    #[test]
    fn test_default_config() {
        let config = WebhookDispatcherConfig::default();
        assert_eq!(config.poll_interval_ms, 5000);
        assert_eq!(config.batch_size, 20);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_retry_delay_secs, 60);
        assert!(config.enabled);
    }

    #[test]
    fn test_custom_config() {
        let config = WebhookDispatcherConfig {
            poll_interval_ms: 1000,
            batch_size: 10,
            max_retries: 3,
            base_retry_delay_secs: 30,
            enabled: false,
        };
        assert_eq!(config.poll_interval_ms, 1000);
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_retry_delay_secs, 30);
        assert!(!config.enabled);
    }

    #[test]
    fn test_exponential_backoff() {
        let base_delay = 60;
        // First retry: 60 * 2^0 = 60 seconds
        assert_eq!(base_delay * 2_i64.pow(0), 60);
        // Second retry: 60 * 2^1 = 120 seconds
        assert_eq!(base_delay * 2_i64.pow(1), 120);
        // Third retry: 60 * 2^2 = 240 seconds
        assert_eq!(base_delay * 2_i64.pow(2), 240);
        // Fourth retry: 60 * 2^3 = 480 seconds
        assert_eq!(base_delay * 2_i64.pow(3), 480);
        // Fifth retry: 60 * 2^4 = 960 seconds
        assert_eq!(base_delay * 2_i64.pow(4), 960);
    }

    // === WebhookSender Tests ===

    #[test]
    fn test_webhook_sender_new() {
        let sender = WebhookSender::new();
        // Just verify it can be created
        assert!(std::mem::size_of_val(&sender) > 0);
    }

    #[test]
    fn test_webhook_sender_default() {
        let sender = WebhookSender::default();
        assert!(std::mem::size_of_val(&sender) > 0);
    }

    // === Signature Generation Tests ===

    #[test]
    fn test_generate_signature() {
        let sender = WebhookSender::new();
        let secret = "test_secret_key";
        let timestamp = 1704067200; // 2024-01-01 00:00:00 UTC
        let body = r#"{"event_type":"action.approved","data":{}}"#;

        let signature = sender.generate_signature(secret, timestamp, body).unwrap();

        // Signature should be a hex-encoded HMAC-SHA256
        assert_eq!(signature.len(), 64); // 32 bytes = 64 hex chars
        assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_signature_deterministic() {
        let sender = WebhookSender::new();
        let secret = "my_secret";
        let timestamp = 1234567890;
        let body = r#"{"test":"data"}"#;

        let sig1 = sender.generate_signature(secret, timestamp, body).unwrap();
        let sig2 = sender.generate_signature(secret, timestamp, body).unwrap();

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_generate_signature_different_secrets() {
        let sender = WebhookSender::new();
        let timestamp = 1234567890;
        let body = r#"{"test":"data"}"#;

        let sig1 = sender.generate_signature("secret1", timestamp, body).unwrap();
        let sig2 = sender.generate_signature("secret2", timestamp, body).unwrap();

        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_generate_signature_different_timestamps() {
        let sender = WebhookSender::new();
        let secret = "my_secret";
        let body = r#"{"test":"data"}"#;

        let sig1 = sender.generate_signature(secret, 1000, body).unwrap();
        let sig2 = sender.generate_signature(secret, 2000, body).unwrap();

        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_generate_signature_different_bodies() {
        let sender = WebhookSender::new();
        let secret = "my_secret";
        let timestamp = 1234567890;

        let sig1 = sender.generate_signature(secret, timestamp, r#"{"a":1}"#).unwrap();
        let sig2 = sender.generate_signature(secret, timestamp, r#"{"a":2}"#).unwrap();

        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_generate_signature_empty_body() {
        let sender = WebhookSender::new();
        let secret = "my_secret";
        let timestamp = 1234567890;

        let signature = sender.generate_signature(secret, timestamp, "").unwrap();
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn test_generate_signature_empty_secret() {
        let sender = WebhookSender::new();
        let timestamp = 1234567890;
        let body = r#"{"test":"data"}"#;

        // Empty secret should still work (HMAC allows it)
        let signature = sender.generate_signature("", timestamp, body).unwrap();
        assert_eq!(signature.len(), 64);
    }

    // === Event Filtering Tests ===

    fn make_tenant(webhook_events: Option<serde_json::Value>) -> Tenant {
        let now = OffsetDateTime::now_utc();
        Tenant {
            id: TenantId::from_string("ten_test".to_string()),
            name: "Test Tenant".to_string(),
            webhook_url: Some("https://example.com/webhook".to_string()),
            webhook_secret: Some("secret".to_string()),
            webhook_events,
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

    #[test]
    fn test_should_send_event_no_filter() {
        let sender = WebhookSender::new();
        let tenant = make_tenant(None);

        // Without a filter, all events should be sent
        assert!(sender.should_send_event(&tenant, WebhookEventType::ApprovalRequired));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionApproved));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionExecuted));
    }

    #[test]
    fn test_should_send_event_with_filter() {
        let sender = WebhookSender::new();
        let tenant = make_tenant(Some(serde_json::json!(["approval_required", "action_approved"])));

        assert!(sender.should_send_event(&tenant, WebhookEventType::ApprovalRequired));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionApproved));
        assert!(!sender.should_send_event(&tenant, WebhookEventType::ActionExecuted));
        assert!(!sender.should_send_event(&tenant, WebhookEventType::ActionDenied));
    }

    #[test]
    fn test_should_send_event_empty_array() {
        let sender = WebhookSender::new();
        let tenant = make_tenant(Some(serde_json::json!([])));

        // Empty array means no events should be sent
        assert!(!sender.should_send_event(&tenant, WebhookEventType::ApprovalRequired));
        assert!(!sender.should_send_event(&tenant, WebhookEventType::ActionApproved));
    }

    #[test]
    fn test_should_send_event_non_array_value() {
        let sender = WebhookSender::new();
        let tenant = make_tenant(Some(serde_json::json!("not_an_array")));

        // Non-array webhook_events defaults to sending all events
        assert!(sender.should_send_event(&tenant, WebhookEventType::ApprovalRequired));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionApproved));
    }

    #[test]
    fn test_should_send_event_all_types() {
        let sender = WebhookSender::new();
        let tenant = make_tenant(Some(serde_json::json!([
            "approval_required",
            "action_approved",
            "action_denied",
            "action_executed",
            "action_failed"
        ])));

        assert!(sender.should_send_event(&tenant, WebhookEventType::ApprovalRequired));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionApproved));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionDenied));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionExecuted));
        assert!(sender.should_send_event(&tenant, WebhookEventType::ActionFailed));
    }
}
