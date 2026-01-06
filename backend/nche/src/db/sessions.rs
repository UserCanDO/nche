use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{ActorType, AgentId, AutonomyLevel, Session, SessionId, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_session(
        &self,
        tenant_id: &TenantId,
        agent_id: &AgentId,
        actor_id: &str,
        actor_type: ActorType,
        autonomy_level: AutonomyLevel,
    ) -> Result<Session> {
        let id = SessionId::new();
        let now = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            INSERT INTO sessions (id, tenant_id, agent_id, actor_id, actor_type, autonomy_level, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(&agent_id.0)
        .bind(actor_id)
        .bind(actor_type_to_str(actor_type))
        .bind(autonomy_level_to_str(autonomy_level))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(Session {
            id,
            tenant_id: tenant_id.clone(),
            agent_id: agent_id.clone(),
            actor_id: actor_id.to_string(),
            actor_type,
            autonomy_level,
            created_at: now,
            ended_at: None,
        })
    }

    pub async fn get_session(
        &self,
        tenant_id: &TenantId,
        session_id: &SessionId,
    ) -> Result<Option<Session>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, agent_id, actor_id, actor_type, autonomy_level, created_at, ended_at
            FROM sessions WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&session_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(|r| Session {
            id: SessionId::from_string(r.get("id")),
            tenant_id: TenantId::from_string(r.get("tenant_id")),
            agent_id: AgentId::from_string(r.get("agent_id")),
            actor_id: r.get("actor_id"),
            actor_type: str_to_actor_type(r.get("actor_type")),
            autonomy_level: str_to_autonomy_level(r.get("autonomy_level")),
            created_at: r.get("created_at"),
            ended_at: r.get("ended_at"),
        }))
    }

    pub async fn end_session(&self, tenant_id: &TenantId, session_id: &SessionId) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE sessions SET ended_at = now()
            WHERE tenant_id = $1 AND id = $2 AND ended_at IS NULL
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&session_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list_sessions(
        &self,
        tenant_id: &TenantId,
        agent_id: Option<&AgentId>,
        active_only: bool,
        limit: i64,
    ) -> Result<Vec<Session>> {
        let mut query = String::from(
            "SELECT id, tenant_id, agent_id, actor_id, actor_type, autonomy_level, created_at, ended_at
             FROM sessions WHERE tenant_id = $1",
        );

        let mut param_idx = 2;

        if agent_id.is_some() {
            query.push_str(&format!(" AND agent_id = ${}", param_idx));
            param_idx += 1;
        }

        if active_only {
            query.push_str(" AND ended_at IS NULL");
        }

        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${}", param_idx));

        let mut q = sqlx::query(&query).bind(&tenant_id.0);

        if let Some(aid) = agent_id {
            q = q.bind(&aid.0);
        }

        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await.map_err(NcheError::Database)?;

        Ok(rows
            .into_iter()
            .map(|r| Session {
                id: SessionId::from_string(r.get("id")),
                tenant_id: TenantId::from_string(r.get("tenant_id")),
                agent_id: AgentId::from_string(r.get("agent_id")),
                actor_id: r.get("actor_id"),
                actor_type: str_to_actor_type(r.get("actor_type")),
                autonomy_level: str_to_autonomy_level(r.get("autonomy_level")),
                created_at: r.get("created_at"),
                ended_at: r.get("ended_at"),
            })
            .collect())
    }
}

fn actor_type_to_str(t: ActorType) -> &'static str {
    match t {
        ActorType::User => "user",
        ActorType::Org => "org",
        ActorType::System => "system",
    }
}

fn str_to_actor_type(s: &str) -> ActorType {
    match s {
        "user" => ActorType::User,
        "org" => ActorType::Org,
        "system" => ActorType::System,
        _ => ActorType::User,
    }
}

fn autonomy_level_to_str(l: AutonomyLevel) -> &'static str {
    match l {
        AutonomyLevel::Full => "full",
        AutonomyLevel::Supervised => "supervised",
        AutonomyLevel::Restricted => "restricted",
    }
}

fn str_to_autonomy_level(s: &str) -> AutonomyLevel {
    match s {
        "full" => AutonomyLevel::Full,
        "supervised" => AutonomyLevel::Supervised,
        "restricted" => AutonomyLevel::Restricted,
        _ => AutonomyLevel::Supervised,
    }
}
