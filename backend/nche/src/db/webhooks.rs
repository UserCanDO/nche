use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{TenantId, WebhookDelivery, WebhookDeliveryId, WebhookDeliveryStatus};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_webhook_delivery(
        &self,
        tenant_id: &TenantId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<WebhookDelivery> {
        let id = WebhookDeliveryId::new();
        let now = OffsetDateTime::now_utc();
        let status = WebhookDeliveryStatus::Pending;

        sqlx::query(
            r#"
            INSERT INTO webhook_deliveries
                (id, tenant_id, event_type, payload, status, attempts, next_attempt_at, attempt_metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, 0, $6, '[]', $7)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(event_type)
        .bind(&payload)
        .bind(webhook_status_to_str(status))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(WebhookDelivery {
            id,
            tenant_id: tenant_id.clone(),
            event_type: event_type.to_string(),
            payload,
            status,
            attempts: 0,
            last_attempt_at: None,
            next_attempt_at: now,
            last_error: None,
            attempt_metadata: serde_json::json!([]),
            created_at: now,
        })
    }

    pub async fn get_webhook_delivery(
        &self,
        delivery_id: &WebhookDeliveryId,
    ) -> Result<Option<WebhookDelivery>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, event_type, payload, status, attempts, last_attempt_at,
                   next_attempt_at, last_error, attempt_metadata, created_at
            FROM webhook_deliveries WHERE id = $1
            "#,
        )
        .bind(&delivery_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(row_to_webhook_delivery))
    }

    /// Get webhook deliveries ready for sending/retry
    pub async fn get_ready_webhook_deliveries(&self, limit: i64) -> Result<Vec<WebhookDelivery>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, event_type, payload, status, attempts, last_attempt_at,
                   next_attempt_at, last_error, attempt_metadata, created_at
            FROM webhook_deliveries
            WHERE status = 'pending' AND next_attempt_at <= now()
            ORDER BY next_attempt_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_webhook_delivery).collect())
    }

    pub async fn update_webhook_delivery_status(
        &self,
        delivery_id: &WebhookDeliveryId,
        status: WebhookDeliveryStatus,
        last_error: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = $2, last_error = $3, last_attempt_at = now()
            WHERE id = $1
            "#,
        )
        .bind(&delivery_id.0)
        .bind(webhook_status_to_str(status))
        .bind(last_error)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn add_webhook_attempt(
        &self,
        delivery_id: &WebhookDeliveryId,
        http_status: Option<u16>,
        duration_ms: u64,
        error: Option<&str>,
    ) -> Result<bool> {
        let attempt_data = serde_json::json!({
            "timestamp": OffsetDateTime::now_utc().to_string(),
            "http_status": http_status,
            "duration_ms": duration_ms,
            "error": error
        });

        let result = sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET attempts = attempts + 1,
                last_attempt_at = now(),
                attempt_metadata = attempt_metadata || $2::jsonb
            WHERE id = $1
            "#,
        )
        .bind(&delivery_id.0)
        .bind(serde_json::json!([attempt_data]))
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn update_webhook_next_attempt(
        &self,
        delivery_id: &WebhookDeliveryId,
        next_attempt_at: OffsetDateTime,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET next_attempt_at = $2, status = 'pending'
            WHERE id = $1
            "#,
        )
        .bind(&delivery_id.0)
        .bind(next_attempt_at)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark delivery as permanently failed (max retries reached)
    pub async fn mark_webhook_failed(&self, delivery_id: &WebhookDeliveryId) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = 'failed'
            WHERE id = $1
            "#,
        )
        .bind(&delivery_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_webhook_delivery(r: sqlx::postgres::PgRow) -> WebhookDelivery {
    WebhookDelivery {
        id: WebhookDeliveryId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        event_type: r.get("event_type"),
        payload: r.get("payload"),
        status: str_to_webhook_status(r.get("status")),
        attempts: r.get("attempts"),
        last_attempt_at: r.get("last_attempt_at"),
        next_attempt_at: r.get("next_attempt_at"),
        last_error: r.get("last_error"),
        attempt_metadata: r.get("attempt_metadata"),
        created_at: r.get("created_at"),
    }
}

fn webhook_status_to_str(s: WebhookDeliveryStatus) -> &'static str {
    match s {
        WebhookDeliveryStatus::Pending => "pending",
        WebhookDeliveryStatus::Delivered => "delivered",
        WebhookDeliveryStatus::Failed => "failed",
    }
}

fn str_to_webhook_status(s: &str) -> WebhookDeliveryStatus {
    match s {
        "pending" => WebhookDeliveryStatus::Pending,
        "delivered" => WebhookDeliveryStatus::Delivered,
        "failed" => WebhookDeliveryStatus::Failed,
        _ => WebhookDeliveryStatus::Pending,
    }
}
