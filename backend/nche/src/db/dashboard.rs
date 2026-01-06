use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{DashboardSession, DashboardSessionId, DashboardUser, DashboardUserId, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

impl Database {
    pub async fn create_dashboard_user(
        &self,
        tenant_id: &TenantId,
        email: &str,
        password: &str,
        name: Option<&str>,
    ) -> Result<DashboardUser> {
        let id = DashboardUserId::new();
        let now = OffsetDateTime::now_utc();

        // Hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| NcheError::Internal(format!("Failed to hash password: {}", e)))?
            .to_string();

        sqlx::query(
            r#"
            INSERT INTO dashboard_users (id, tenant_id, email, password_hash, name, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&id.0)
        .bind(&tenant_id.0)
        .bind(email)
        .bind(&password_hash)
        .bind(name)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(DashboardUser {
            id,
            tenant_id: tenant_id.clone(),
            email: email.to_string(),
            password_hash,
            name: name.map(String::from),
            created_at: now,
        })
    }

    pub async fn get_dashboard_user_by_email(
        &self,
        email: &str,
    ) -> Result<Option<DashboardUser>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, email, password_hash, name, created_at
            FROM dashboard_users WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(|r| DashboardUser {
            id: DashboardUserId::from_string(r.get("id")),
            tenant_id: TenantId::from_string(r.get("tenant_id")),
            email: r.get("email"),
            password_hash: r.get("password_hash"),
            name: r.get("name"),
            created_at: r.get("created_at"),
        }))
    }

    /// Verify password and return user if valid
    /// Looks up user by email and verifies password, returning user with their tenant_id
    pub async fn verify_dashboard_user(
        &self,
        email: &str,
        password: &str,
    ) -> Result<Option<DashboardUser>> {
        let user = match self.get_dashboard_user_by_email(email).await? {
            Some(u) => u,
            None => return Ok(None),
        };

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| NcheError::Internal(format!("Invalid hash format: {}", e)))?;

        let matches = Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok();

        if matches {
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    pub async fn create_dashboard_session(
        &self,
        user_id: &DashboardUserId,
        tenant_id: &TenantId,
        expires_at: OffsetDateTime,
    ) -> Result<DashboardSession> {
        let id = DashboardSessionId::new();
        let now = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            INSERT INTO dashboard_sessions (id, user_id, tenant_id, expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&id.0)
        .bind(&user_id.0)
        .bind(&tenant_id.0)
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(DashboardSession {
            id,
            user_id: user_id.clone(),
            tenant_id: tenant_id.clone(),
            expires_at,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn get_dashboard_session(
        &self,
        session_id: &DashboardSessionId,
    ) -> Result<Option<DashboardSession>> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, tenant_id, expires_at, created_at, updated_at
            FROM dashboard_sessions WHERE id = $1 AND expires_at > now()
            "#,
        )
        .bind(&session_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(|r| DashboardSession {
            id: DashboardSessionId::from_string(r.get("id")),
            user_id: DashboardUserId::from_string(r.get("user_id")),
            tenant_id: TenantId::from_string(r.get("tenant_id")),
            expires_at: r.get("expires_at"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    pub async fn delete_dashboard_session(
        &self,
        session_id: &DashboardSessionId,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM dashboard_sessions WHERE id = $1
            "#,
        )
        .bind(&session_id.0)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    /// Extend session expiry (touch)
    pub async fn extend_dashboard_session(
        &self,
        session_id: &DashboardSessionId,
        new_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE dashboard_sessions
            SET expires_at = $2, updated_at = now()
            WHERE id = $1 AND expires_at > now()
            "#,
        )
        .bind(&session_id.0)
        .bind(new_expires_at)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected() > 0)
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM dashboard_sessions WHERE expires_at <= now()
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(result.rows_affected())
    }
}
