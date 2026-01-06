//! Webhook integration tests.
//!
//! Tests webhook delivery, retry logic, and signature generation.
//!
//! # Prerequisites
//!
//! Set the TEST_DATABASE_URL environment variable or use the default:
//! `postgres://postgres:postgres@localhost:5432/nche_test`
//!
//! # Running
//!
//! ```bash
//! cargo test --test webhook_test
//! ```

mod common;

use nche::domain::*;
use time::{Duration, OffsetDateTime};

// ============================================================================
// Webhook Delivery CRUD Tests
// ============================================================================

#[tokio::test]
async fn test_webhook_delivery_crud() {
    let ctx = common::TestContext::new().await;

    // Create delivery
    let payload = serde_json::json!({
        "action_id": "act_test123",
        "tool": "send_email"
    });
    let delivery = ctx
        .db
        .create_webhook_delivery(&ctx.tenant.id, "action_created", payload.clone())
        .await
        .unwrap();

    assert!(!delivery.id.0.is_empty());
    assert_eq!(delivery.event_type, "action_created");
    assert_eq!(delivery.status, WebhookDeliveryStatus::Pending);
    assert_eq!(delivery.attempts, 0);

    // Get delivery
    let fetched = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id.0, delivery.id.0);
    assert_eq!(fetched.payload, payload);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_webhook_delivery_status_update() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "approval_required",
            serde_json::json!({"approval_id": "appr_test"}),
        )
        .await
        .unwrap();

    // Update to delivered
    ctx.db
        .update_webhook_delivery_status(&delivery.id, WebhookDeliveryStatus::Delivered, None)
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, WebhookDeliveryStatus::Delivered);
    assert!(updated.last_attempt_at.is_some());

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_webhook_delivery_failed_with_error() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "action_executed",
            serde_json::json!({"result": "success"}),
        )
        .await
        .unwrap();

    // Update with error
    ctx.db
        .update_webhook_delivery_status(
            &delivery.id,
            WebhookDeliveryStatus::Failed,
            Some("Connection refused"),
        )
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, WebhookDeliveryStatus::Failed);
    assert_eq!(updated.last_error, Some("Connection refused".to_string()));

    ctx.cleanup().await;
}

// ============================================================================
// Ready Delivery Selection Tests
// ============================================================================

#[tokio::test]
async fn test_get_ready_webhook_deliveries() {
    let ctx = common::TestContext::new().await;

    // Create some deliveries
    let delivery1 = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "event_1",
            serde_json::json!({"seq": 1}),
        )
        .await
        .unwrap();

    let delivery2 = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "event_2",
            serde_json::json!({"seq": 2}),
        )
        .await
        .unwrap();

    // Both should be ready (next_attempt_at is set to now)
    let ready = ctx.db.get_ready_webhook_deliveries(10).await.unwrap();

    // Filter to only our deliveries (other tests may have left some)
    let our_deliveries: Vec<_> = ready
        .iter()
        .filter(|d| d.tenant_id.0 == ctx.tenant.id.0)
        .collect();
    assert!(our_deliveries.len() >= 2);
    assert!(our_deliveries.iter().any(|d| d.id.0 == delivery1.id.0));
    assert!(our_deliveries.iter().any(|d| d.id.0 == delivery2.id.0));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_get_ready_webhook_deliveries_excludes_delivered() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "delivered_event",
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // Mark as delivered
    ctx.db
        .update_webhook_delivery_status(&delivery.id, WebhookDeliveryStatus::Delivered, None)
        .await
        .unwrap();

    // Should not appear in ready list
    let ready = ctx.db.get_ready_webhook_deliveries(100).await.unwrap();
    assert!(!ready.iter().any(|d| d.id.0 == delivery.id.0));

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_get_ready_webhook_deliveries_excludes_future() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "future_event",
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // Set next_attempt_at to future
    let future_time = OffsetDateTime::now_utc() + Duration::hours(1);
    ctx.db
        .update_webhook_next_attempt(&delivery.id, future_time)
        .await
        .unwrap();

    // Should not appear in ready list
    let ready = ctx.db.get_ready_webhook_deliveries(100).await.unwrap();
    assert!(!ready.iter().any(|d| d.id.0 == delivery.id.0));

    ctx.cleanup().await;
}

// ============================================================================
// Retry and Attempt Tracking Tests
// ============================================================================

#[tokio::test]
async fn test_webhook_attempt_tracking() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "retry_event",
            serde_json::json!({"data": "test"}),
        )
        .await
        .unwrap();

    // Add first attempt (failed)
    ctx.db
        .add_webhook_attempt(&delivery.id, Some(500), 150, Some("Server error"))
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.attempts, 1);
    assert!(updated.last_attempt_at.is_some());

    // Check attempt metadata
    let metadata = updated.attempt_metadata.as_array().unwrap();
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0]["http_status"], 500);
    assert_eq!(metadata[0]["duration_ms"], 150);
    assert_eq!(metadata[0]["error"], "Server error");

    // Add second attempt (failed)
    ctx.db
        .add_webhook_attempt(&delivery.id, Some(502), 200, Some("Bad gateway"))
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.attempts, 2);
    assert_eq!(updated.attempt_metadata.as_array().unwrap().len(), 2);

    // Add third attempt (success - no error)
    ctx.db
        .add_webhook_attempt(&delivery.id, Some(200), 50, None)
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.attempts, 3);

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_webhook_next_attempt_scheduling() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "schedule_event",
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // Schedule retry in 60 seconds
    let next_time = OffsetDateTime::now_utc() + Duration::seconds(60);
    ctx.db
        .update_webhook_next_attempt(&delivery.id, next_time)
        .await
        .unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();

    // Should still be pending
    assert_eq!(updated.status, WebhookDeliveryStatus::Pending);

    // Next attempt should be in the future
    assert!(updated.next_attempt_at > OffsetDateTime::now_utc());

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_webhook_mark_failed() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "max_retries_event",
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // Simulate max retries reached
    for i in 1..=5 {
        ctx.db
            .add_webhook_attempt(&delivery.id, Some(500), 100, Some(&format!("Attempt {} failed", i)))
            .await
            .unwrap();
    }

    // Mark as permanently failed
    ctx.db.mark_webhook_failed(&delivery.id).await.unwrap();

    let updated = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, WebhookDeliveryStatus::Failed);
    assert_eq!(updated.attempts, 5);

    ctx.cleanup().await;
}

// ============================================================================
// Webhook Queuing Tests
// ============================================================================

#[tokio::test]
async fn test_webhook_event_types() {
    let ctx = common::TestContext::new().await;

    // Test all webhook event types
    let event_types = vec![
        "approval_required",
        "action_approved",
        "action_denied",
        "action_executed",
        "action_failed",
    ];

    for event_type in event_types {
        let delivery = ctx
            .db
            .create_webhook_delivery(
                &ctx.tenant.id,
                event_type,
                serde_json::json!({"test": true}),
            )
            .await
            .unwrap();

        assert_eq!(delivery.event_type, event_type);
        assert_eq!(delivery.status, WebhookDeliveryStatus::Pending);
    }

    ctx.cleanup().await;
}

#[tokio::test]
async fn test_webhook_payload_preservation() {
    let ctx = common::TestContext::new().await;

    let complex_payload = serde_json::json!({
        "action": {
            "id": "act_123",
            "tool": "send_email",
            "params": {
                "to": "user@example.com",
                "subject": "Test",
                "body": "Hello with special chars: <>&'\""
            }
        },
        "session": {
            "id": "sess_456",
            "autonomy_level": "supervised"
        },
        "nested": {
            "deep": {
                "value": [1, 2, 3]
            }
        }
    });

    let delivery = ctx
        .db
        .create_webhook_delivery(&ctx.tenant.id, "action_created", complex_payload.clone())
        .await
        .unwrap();

    let fetched = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();

    // Verify payload is preserved exactly
    assert_eq!(fetched.payload, complex_payload);
    assert_eq!(fetched.payload["action"]["params"]["to"], "user@example.com");
    assert_eq!(fetched.payload["nested"]["deep"]["value"], serde_json::json!([1, 2, 3]));

    ctx.cleanup().await;
}

// ============================================================================
// Exponential Backoff Calculation Tests
// ============================================================================

#[tokio::test]
async fn test_exponential_backoff_simulation() {
    let ctx = common::TestContext::new().await;

    let delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "backoff_test",
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // Simulate exponential backoff: 60s, 120s, 240s, 480s, 960s
    let base_delay = 60i64;
    let delays = vec![60, 120, 240, 480, 960];

    for (attempt, expected_delay) in delays.iter().enumerate() {
        let attempt_num = attempt as i32;
        let actual_delay = base_delay * 2i64.pow(attempt_num as u32);
        assert_eq!(actual_delay, *expected_delay);

        // Schedule next attempt
        let next_time = OffsetDateTime::now_utc() + Duration::seconds(actual_delay);
        ctx.db
            .update_webhook_next_attempt(&delivery.id, next_time)
            .await
            .unwrap();

        // Record the attempt
        ctx.db
            .add_webhook_attempt(&delivery.id, Some(500), 100, Some("Server error"))
            .await
            .unwrap();
    }

    let final_delivery = ctx
        .db
        .get_webhook_delivery(&delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_delivery.attempts, 5);

    ctx.cleanup().await;
}

// ============================================================================
// Concurrent Delivery Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_deliveries_ordering() {
    let ctx = common::TestContext::new().await;

    // Create deliveries with staggered next_attempt_at times
    let mut deliveries = vec![];
    for i in 0..5 {
        let delivery = ctx
            .db
            .create_webhook_delivery(
                &ctx.tenant.id,
                &format!("ordered_event_{}", i),
                serde_json::json!({"seq": i}),
            )
            .await
            .unwrap();

        // Set next_attempt_at with increasing delay
        let delay = Duration::seconds(i as i64);
        let next_time = OffsetDateTime::now_utc() - Duration::minutes(5) + delay;
        ctx.db
            .update_webhook_next_attempt(&delivery.id, next_time)
            .await
            .unwrap();

        deliveries.push(delivery);
    }

    // Get ready deliveries - should be ordered by next_attempt_at ASC
    let ready = ctx.db.get_ready_webhook_deliveries(10).await.unwrap();

    // Filter to our deliveries
    let our_ready: Vec<_> = ready
        .iter()
        .filter(|d| d.event_type.starts_with("ordered_event_"))
        .collect();

    // Verify ordering (earlier next_attempt_at first)
    for i in 1..our_ready.len() {
        assert!(
            our_ready[i - 1].next_attempt_at <= our_ready[i].next_attempt_at,
            "Deliveries should be ordered by next_attempt_at"
        );
    }

    ctx.cleanup().await;
}

// ============================================================================
// Tenant-Scoped Delivery Tests
// ============================================================================

#[tokio::test]
async fn test_deliveries_tenant_isolated() {
    let ctx = common::TestContext::new().await;

    // Create another tenant
    let other_tenant = ctx
        .db
        .create_tenant("Other Tenant", None, None, None, None)
        .await
        .unwrap();

    // Create delivery for main tenant
    let our_delivery = ctx
        .db
        .create_webhook_delivery(
            &ctx.tenant.id,
            "our_event",
            serde_json::json!({"tenant": "main"}),
        )
        .await
        .unwrap();

    // Create delivery for other tenant
    let other_delivery = ctx
        .db
        .create_webhook_delivery(
            &other_tenant.id,
            "other_event",
            serde_json::json!({"tenant": "other"}),
        )
        .await
        .unwrap();

    // Both should be in ready deliveries (they're global)
    let ready = ctx.db.get_ready_webhook_deliveries(100).await.unwrap();

    assert!(ready.iter().any(|d| d.id.0 == our_delivery.id.0));
    assert!(ready.iter().any(|d| d.id.0 == other_delivery.id.0));

    // But they have different tenant_ids
    let our = ready.iter().find(|d| d.id.0 == our_delivery.id.0).unwrap();
    let other = ready.iter().find(|d| d.id.0 == other_delivery.id.0).unwrap();

    assert_eq!(our.tenant_id.0, ctx.tenant.id.0);
    assert_eq!(other.tenant_id.0, other_tenant.id.0);

    // Cleanup other tenant
    let _ = sqlx::query("DELETE FROM webhook_deliveries WHERE tenant_id = $1")
        .bind(&other_tenant.id.0)
        .execute(&ctx.db.pool)
        .await;
    let _ = sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(&other_tenant.id.0)
        .execute(&ctx.db.pool)
        .await;

    ctx.cleanup().await;
}
