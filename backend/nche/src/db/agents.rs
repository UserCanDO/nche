use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::Row;
use time::OffsetDateTime;

use crate::domain::{Agent, AgentId, TenantId};
use crate::error::{NcheError, Result};

use super::Database;

/// API key structure: nche_<agent_id>_<secret>
pub struct ApiKey {
    pub full_key: String,
    pub prefix: String,
    pub secret: String,
}

impl ApiKey {
    pub fn generate(agent_id: &AgentId) -> Self {
        let secret = nanoid::nanoid!(24);
        let prefix = format!("nche_{}", agent_id.0);
        let full_key = format!("{}_{}", prefix, secret);

        Self {
            full_key,
            prefix,
            secret,
        }
    }

    pub fn parse(key: &str) -> Option<(String, String)> {
        // Format: nche_<agent_id>_<secret>
        // Secret is always 24 chars, so extract from the end
        if !key.starts_with("nche_agt_") {
            return None;
        }

        // Secret is 24 chars at the end, plus underscore separator
        const SECRET_LEN: usize = 24;
        if key.len() < 5 + 4 + 1 + 1 + SECRET_LEN {
            // nche_ (5) + agt_ (4) + at least 1 char + _ + secret
            return None;
        }

        let secret_start = key.len() - SECRET_LEN;
        let separator_pos = secret_start - 1;

        // Check separator is underscore
        if key.as_bytes()[separator_pos] != b'_' {
            return None;
        }

        let prefix = key[..separator_pos].to_string();
        let secret = key[secret_start..].to_string();

        Some((prefix, secret))
    }
}

impl Database {
    /// Create an agent and return the plaintext API key (only shown once)
    pub async fn create_agent_with_key(
        &self,
        tenant_id: &TenantId,
        name: &str,
    ) -> Result<(Agent, String)> {
        let agent_id = AgentId::new();
        let api_key = ApiKey::generate(&agent_id);
        let now = OffsetDateTime::now_utc();

        // Hash the secret portion
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let api_key_hash = argon2
            .hash_password(api_key.secret.as_bytes(), &salt)
            .map_err(|e| NcheError::Internal(format!("Failed to hash API key: {}", e)))?
            .to_string();

        sqlx::query(
            r#"
            INSERT INTO agents (id, tenant_id, name, api_key_hash, api_key_prefix, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&agent_id.0)
        .bind(&tenant_id.0)
        .bind(name)
        .bind(&api_key_hash)
        .bind(&api_key.prefix)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        let agent = Agent {
            id: agent_id,
            tenant_id: tenant_id.clone(),
            name: name.to_string(),
            api_key_hash,
            api_key_prefix: api_key.prefix,
            created_at: now,
        };

        Ok((agent, api_key.full_key))
    }

    /// Verify an API key and return the agent + tenant_id
    pub async fn get_agent_by_api_key(&self, api_key: &str) -> Result<Option<(Agent, TenantId)>> {
        let (prefix, secret) = match ApiKey::parse(api_key) {
            Some(parsed) => parsed,
            None => return Ok(None),
        };

        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, name, api_key_hash, api_key_prefix, created_at
            FROM agents WHERE api_key_prefix = $1
            "#,
        )
        .bind(&prefix)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        let Some(row) = row else {
            return Ok(None);
        };

        let agent = Agent {
            id: AgentId::from_string(row.get("id")),
            tenant_id: TenantId::from_string(row.get("tenant_id")),
            name: row.get("name"),
            api_key_hash: row.get("api_key_hash"),
            api_key_prefix: row.get("api_key_prefix"),
            created_at: row.get("created_at"),
        };

        // Verify the secret against the hash
        let parsed_hash = PasswordHash::new(&agent.api_key_hash)
            .map_err(|e| NcheError::Internal(format!("Invalid hash format: {}", e)))?;

        let matches = Argon2::default()
            .verify_password(secret.as_bytes(), &parsed_hash)
            .is_ok();

        if matches {
            let tenant_id = agent.tenant_id.clone();
            Ok(Some((agent, tenant_id)))
        } else {
            Ok(None)
        }
    }

    pub async fn get_agent(&self, tenant_id: &TenantId, agent_id: &AgentId) -> Result<Option<Agent>> {
        let row = sqlx::query(
            r#"
            SELECT id, tenant_id, name, api_key_hash, api_key_prefix, created_at
            FROM agents WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(&agent_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(row.map(|r| Agent {
            id: AgentId::from_string(r.get("id")),
            tenant_id: TenantId::from_string(r.get("tenant_id")),
            name: r.get("name"),
            api_key_hash: r.get("api_key_hash"),
            api_key_prefix: r.get("api_key_prefix"),
            created_at: r.get("created_at"),
        }))
    }

    pub async fn list_agents(&self, tenant_id: &TenantId, limit: i64) -> Result<Vec<Agent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, tenant_id, name, api_key_hash, api_key_prefix, created_at
            FROM agents WHERE tenant_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(&tenant_id.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(NcheError::Database)?;

        Ok(rows
            .into_iter()
            .map(|r| Agent {
                id: AgentId::from_string(r.get("id")),
                tenant_id: TenantId::from_string(r.get("tenant_id")),
                name: r.get("name"),
                api_key_hash: r.get("api_key_hash"),
                api_key_prefix: r.get("api_key_prefix"),
                created_at: r.get("created_at"),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === API Key Generation Tests ===

    #[test]
    fn test_api_key_generate() {
        let agent_id = AgentId::from_string("agt_test123456".to_string());
        let key = ApiKey::generate(&agent_id);

        assert!(key.full_key.starts_with("nche_agt_test123456_"));
        assert_eq!(key.prefix, "nche_agt_test123456");
        assert_eq!(key.secret.len(), 24); // nanoid!(24)
    }

    #[test]
    fn test_api_key_format() {
        let agent_id = AgentId::from_string("agt_abc123".to_string());
        let key = ApiKey::generate(&agent_id);

        // Key should be: nche_<agent_id>_<secret>
        let parts: Vec<&str> = key.full_key.splitn(2, '_').collect();
        assert_eq!(parts[0], "nche");

        // Full key should contain the agent ID
        assert!(key.full_key.contains("agt_abc123"));
    }

    // === API Key Parsing Tests ===

    #[test]
    fn test_api_key_parse_valid() {
        let key = "nche_agt_test123456_secretsecretsecretsecret";
        let result = ApiKey::parse(key);

        assert!(result.is_some());
        let (prefix, secret) = result.unwrap();
        assert_eq!(prefix, "nche_agt_test123456");
        assert_eq!(secret, "secretsecretsecretsecret");
    }

    #[test]
    fn test_api_key_parse_missing_prefix() {
        let key = "agt_test123456_secretsecret";
        let result = ApiKey::parse(key);
        assert!(result.is_none());
    }

    #[test]
    fn test_api_key_parse_wrong_prefix() {
        let key = "other_agt_test123456_secret";
        let result = ApiKey::parse(key);
        assert!(result.is_none());
    }

    #[test]
    fn test_api_key_parse_empty_secret() {
        let key = "nche_agt_test123456_";
        let result = ApiKey::parse(key);
        assert!(result.is_none());
    }

    #[test]
    fn test_api_key_parse_no_agent_prefix() {
        let key = "nche_test123456_secret";
        let result = ApiKey::parse(key);
        assert!(result.is_none()); // Missing "agt_" in the prefix
    }

    #[test]
    fn test_api_key_parse_too_short() {
        let key = "nche_";
        let result = ApiKey::parse(key);
        assert!(result.is_none());
    }

    #[test]
    fn test_api_key_parse_empty() {
        let key = "";
        let result = ApiKey::parse(key);
        assert!(result.is_none());
    }

    #[test]
    fn test_api_key_parse_no_underscore() {
        let key = "ncheagttestsecret";
        let result = ApiKey::parse(key);
        assert!(result.is_none());
    }

    #[test]
    fn test_api_key_roundtrip() {
        let agent_id = AgentId::from_string("agt_roundtrip12".to_string());
        let key = ApiKey::generate(&agent_id);

        let parsed = ApiKey::parse(&key.full_key);
        assert!(parsed.is_some());

        let (prefix, secret) = parsed.unwrap();
        assert_eq!(prefix, key.prefix);
        assert_eq!(secret, key.secret);
    }

    #[test]
    fn test_api_key_uniqueness() {
        let agent_id = AgentId::from_string("agt_same123456".to_string());
        let key1 = ApiKey::generate(&agent_id);
        let key2 = ApiKey::generate(&agent_id);

        // Same agent ID but different secrets
        assert_eq!(key1.prefix, key2.prefix);
        assert_ne!(key1.secret, key2.secret);
        assert_ne!(key1.full_key, key2.full_key);
    }
}
