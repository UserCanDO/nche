use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{Tenant, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_tenant(
        &self,
        name: &str,
        webhook_url: Option<&str>,
        webhook_secret: Option<&str>,
        webhook_events: Option<serde_json::Value>,
        internal_domains: Option<serde_json::Value>,
    ) -> Result<Tenant> {
        let id = TenantId::new();
        let now = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            INSERT INTO tenants (id, name, webhook_url, webhook_secret, webhook_events, internal_domains, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&id.0)
        .bind(name)
        .bind(webhook_url)
        .bind(webhook_secret)
        .bind(&webhook_events)
        .bind(&internal_domains)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Tenant {
            id,
            name: name.to_string(),
            webhook_url: webhook_url.map(String::from),
            webhook_secret: webhook_secret.map(String::from),
            webhook_events,
            internal_domains,
            execution_webhook_url: None,
            execution_webhook_secret: None,
            execution_webhook_timeout_ms: None,
            policy_mode: Some("builtin".to_string()),
            policy_webhook_url: None,
            policy_webhook_secret: None,
            policy_webhook_timeout_ms: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_tenant(&self, id: &TenantId) -> Result<Option<Tenant>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, webhook_url, webhook_secret, webhook_events, internal_domains,
                   execution_webhook_url, execution_webhook_secret, execution_webhook_timeout_ms,
                   policy_mode, policy_webhook_url, policy_webhook_secret, policy_webhook_timeout_ms,
                   created_at, updated_at
            FROM tenants WHERE id = $1
            "#,
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(|r| Tenant {
            id: TenantId::from_string(r.get("id")),
            name: r.get("name"),
            webhook_url: r.get("webhook_url"),
            webhook_secret: r.get("webhook_secret"),
            webhook_events: r.get("webhook_events"),
            internal_domains: r.get("internal_domains"),
            execution_webhook_url: r.get("execution_webhook_url"),
            execution_webhook_secret: r.get("execution_webhook_secret"),
            execution_webhook_timeout_ms: r.get("execution_webhook_timeout_ms"),
            policy_mode: r.get("policy_mode"),
            policy_webhook_url: r.get("policy_webhook_url"),
            policy_webhook_secret: r.get("policy_webhook_secret"),
            policy_webhook_timeout_ms: r.get("policy_webhook_timeout_ms"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    pub async fn list_tenants(&self, limit: i64) -> Result<Vec<Tenant>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, webhook_url, webhook_secret, webhook_events, internal_domains,
                   execution_webhook_url, execution_webhook_secret, execution_webhook_timeout_ms,
                   policy_mode, policy_webhook_url, policy_webhook_secret, policy_webhook_timeout_ms,
                   created_at, updated_at
            FROM tenants
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows
            .into_iter()
            .map(|r| Tenant {
                id: TenantId::from_string(r.get("id")),
                name: r.get("name"),
                webhook_url: r.get("webhook_url"),
                webhook_secret: r.get("webhook_secret"),
                webhook_events: r.get("webhook_events"),
                internal_domains: r.get("internal_domains"),
                execution_webhook_url: r.get("execution_webhook_url"),
                execution_webhook_secret: r.get("execution_webhook_secret"),
                execution_webhook_timeout_ms: r.get("execution_webhook_timeout_ms"),
                policy_mode: r.get("policy_mode"),
                policy_webhook_url: r.get("policy_webhook_url"),
                policy_webhook_secret: r.get("policy_webhook_secret"),
                policy_webhook_timeout_ms: r.get("policy_webhook_timeout_ms"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    pub async fn update_tenant(
        &self,
        id: &TenantId,
        name: Option<&str>,
        webhook_url: Option<Option<&str>>,
        webhook_secret: Option<Option<&str>>,
        webhook_events: Option<Option<serde_json::Value>>,
        internal_domains: Option<Option<serde_json::Value>>,
    ) -> Result<Option<Tenant>> {
        // Build dynamic update query
        let mut updates = Vec::new();
        let mut param_idx = 2;

        if name.is_some() {
            updates.push(format!("name = ${}", param_idx));
            param_idx += 1;
        }
        if webhook_url.is_some() {
            updates.push(format!("webhook_url = ${}", param_idx));
            param_idx += 1;
        }
        if webhook_secret.is_some() {
            updates.push(format!("webhook_secret = ${}", param_idx));
            param_idx += 1;
        }
        if webhook_events.is_some() {
            updates.push(format!("webhook_events = ${}", param_idx));
            param_idx += 1;
        }
        if internal_domains.is_some() {
            updates.push(format!("internal_domains = ${}", param_idx));
        }

        if updates.is_empty() {
            return self.get_tenant(id).await;
        }

        let query = format!(
            "UPDATE tenants SET {}, updated_at = now() WHERE id = $1
             RETURNING id, name, webhook_url, webhook_secret, webhook_events, internal_domains,
                       execution_webhook_url, execution_webhook_secret, execution_webhook_timeout_ms,
                       policy_mode, policy_webhook_url, policy_webhook_secret, policy_webhook_timeout_ms,
                       created_at, updated_at",
            updates.join(", ")
        );

        let mut q = sqlx::query(&query).bind(&id.0);

        if let Some(n) = name {
            q = q.bind(n);
        }
        if let Some(url) = webhook_url {
            q = q.bind(url);
        }
        if let Some(secret) = webhook_secret {
            q = q.bind(secret);
        }
        if let Some(events) = webhook_events {
            q = q.bind(events);
        }
        if let Some(domains) = internal_domains {
            q = q.bind(domains);
        }

        let row = q
            .fetch_optional(&self.pool)
            .await
            .map_err(NcheError::Database)?;

        Ok(row.map(|r| Tenant {
            id: TenantId::from_string(r.get("id")),
            name: r.get("name"),
            webhook_url: r.get("webhook_url"),
            webhook_secret: r.get("webhook_secret"),
            webhook_events: r.get("webhook_events"),
            internal_domains: r.get("internal_domains"),
            execution_webhook_url: r.get("execution_webhook_url"),
            execution_webhook_secret: r.get("execution_webhook_secret"),
            execution_webhook_timeout_ms: r.get("execution_webhook_timeout_ms"),
            policy_mode: r.get("policy_mode"),
            policy_webhook_url: r.get("policy_webhook_url"),
            policy_webhook_secret: r.get("policy_webhook_secret"),
            policy_webhook_timeout_ms: r.get("policy_webhook_timeout_ms"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    /// Update tenant execution and policy webhook configuration.
    ///
    /// All fields are optional - only provided fields are updated.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_tenant_config(
        &self,
        id: &TenantId,
        execution_webhook_url: Option<Option<&str>>,
        execution_webhook_secret: Option<Option<&str>>,
        execution_webhook_timeout_ms: Option<Option<i32>>,
        policy_mode: Option<&str>,
        policy_webhook_url: Option<Option<&str>>,
        policy_webhook_secret: Option<Option<&str>>,
        policy_webhook_timeout_ms: Option<Option<i32>>,
    ) -> Result<Option<Tenant>> {
        let mut updates = Vec::new();
        let mut param_idx = 2;

        if execution_webhook_url.is_some() {
            updates.push(format!("execution_webhook_url = ${}", param_idx));
            param_idx += 1;
        }
        if execution_webhook_secret.is_some() {
            updates.push(format!("execution_webhook_secret = ${}", param_idx));
            param_idx += 1;
        }
        if execution_webhook_timeout_ms.is_some() {
            updates.push(format!("execution_webhook_timeout_ms = ${}", param_idx));
            param_idx += 1;
        }
        if policy_mode.is_some() {
            updates.push(format!("policy_mode = ${}", param_idx));
            param_idx += 1;
        }
        if policy_webhook_url.is_some() {
            updates.push(format!("policy_webhook_url = ${}", param_idx));
            param_idx += 1;
        }
        if policy_webhook_secret.is_some() {
            updates.push(format!("policy_webhook_secret = ${}", param_idx));
            param_idx += 1;
        }
        if policy_webhook_timeout_ms.is_some() {
            updates.push(format!("policy_webhook_timeout_ms = ${}", param_idx));
            let _ = param_idx; // Suppress unused warning for last increment
        }

        if updates.is_empty() {
            return self.get_tenant(id).await;
        }

        let query = format!(
            "UPDATE tenants SET {}, updated_at = now() WHERE id = $1
             RETURNING id, name, webhook_url, webhook_secret, webhook_events, internal_domains,
                       execution_webhook_url, execution_webhook_secret, execution_webhook_timeout_ms,
                       policy_mode, policy_webhook_url, policy_webhook_secret, policy_webhook_timeout_ms,
                       created_at, updated_at",
            updates.join(", ")
        );

        let mut q = sqlx::query(&query).bind(&id.0);

        if let Some(url) = execution_webhook_url {
            q = q.bind(url);
        }
        if let Some(secret) = execution_webhook_secret {
            q = q.bind(secret);
        }
        if let Some(timeout) = execution_webhook_timeout_ms {
            q = q.bind(timeout);
        }
        if let Some(mode) = policy_mode {
            q = q.bind(mode);
        }
        if let Some(url) = policy_webhook_url {
            q = q.bind(url);
        }
        if let Some(secret) = policy_webhook_secret {
            q = q.bind(secret);
        }
        if let Some(timeout) = policy_webhook_timeout_ms {
            q = q.bind(timeout);
        }

        let row = q
            .fetch_optional(&self.pool)
            .await
            .map_err(NcheError::Database)?;

        Ok(row.map(|r| Tenant {
            id: TenantId::from_string(r.get("id")),
            name: r.get("name"),
            webhook_url: r.get("webhook_url"),
            webhook_secret: r.get("webhook_secret"),
            webhook_events: r.get("webhook_events"),
            internal_domains: r.get("internal_domains"),
            execution_webhook_url: r.get("execution_webhook_url"),
            execution_webhook_secret: r.get("execution_webhook_secret"),
            execution_webhook_timeout_ms: r.get("execution_webhook_timeout_ms"),
            policy_mode: r.get("policy_mode"),
            policy_webhook_url: r.get("policy_webhook_url"),
            policy_webhook_secret: r.get("policy_webhook_secret"),
            policy_webhook_timeout_ms: r.get("policy_webhook_timeout_ms"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }
}
