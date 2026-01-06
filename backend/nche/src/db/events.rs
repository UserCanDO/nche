use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{ActionId, Event, EventId, SessionId, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_event(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        action_id: Option<&ActionId>,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<Event> {
        let id = EventId::new();
        let now = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            INSERT INTO events (id, tenant_id, session_id, action_id, event_type, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(session_id.map(|s| &s.0))
        .bind(action_id.map(|a| &a.0))
        .bind(event_type)
        .bind(&payload)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Event {
            id,
            tenant_id: tenant_id.clone(),
            session_id: session_id.cloned(),
            action_id: action_id.cloned(),
            event_type: event_type.to_string(),
            payload,
            created_at: now,
        })
    }

    pub async fn list_events(
        &self,
        tenant_id: &TenantId,
        session_id: Option<&SessionId>,
        action_id: Option<&ActionId>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Event>> {
        let mut query = String::from(
            "SELECT id, tenant_id, session_id, action_id, event_type, payload, created_at
             FROM events WHERE tenant_id = $1",
        );

        let mut param_idx = 2;

        if session_id.is_some() {
            query.push_str(&format!(" AND session_id = ${}", param_idx));
            param_idx += 1;
        }

        if action_id.is_some() {
            query.push_str(&format!(" AND action_id = ${}", param_idx));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query(&query).bind(&tenant_id.0);

        if let Some(sid) = session_id {
            q = q.bind(&sid.0);
        }

        if let Some(aid) = action_id {
            q = q.bind(&aid.0);
        }

        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_event).collect())
    }

    /// Get events for an action (timeline view)
    pub async fn get_action_events(
        &self,
        tenant_id: &TenantId,
        action_id: &ActionId,
    ) -> Result<Vec<Event>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, session_id, action_id, event_type, payload, created_at
            FROM events
            WHERE tenant_id = $1 AND action_id = $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&action_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows.into_iter().map(row_to_event).collect())
    }
}

fn row_to_event(r: sqlx::postgres::PgRow) -> Event {
    Event {
        id: EventId::from_string(r.get("id")),
        tenant_id: TenantId::from_string(r.get("tenant_id")),
        session_id: r
            .get::<Option<String>, _>("session_id")
            .map(SessionId::from_string),
        action_id: r
            .get::<Option<String>, _>("action_id")
            .map(ActionId::from_string),
        event_type: r.get("event_type"),
        payload: r.get("payload"),
        created_at: r.get("created_at"),
    }
}
