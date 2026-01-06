//! Common test utilities and fixtures for integration tests.
//!
//! Provides helpers for:
//! - Database setup and cleanup
//! - Creating test fixtures (tenants, agents, sessions, actions)
//! - Building test HTTP clients
//!
//! # Usage
//!
//! ```rust,ignore
//! use tests::common::TestContext;
//!
//! #[tokio::test]
//! async fn test_something() {
//!     let ctx = TestContext::new().await;
//!     // Use ctx.db, ctx.tenant, ctx.agent, ctx.api_key...
//! }
//! ```

use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    Router,
};
use nche::{
    api::{create_router, AppState},
    db::Database,
    domain::*,
};
use tower::ServiceExt;

/// Test context with database connection and pre-created fixtures.
pub struct TestContext {
    pub db: Arc<Database>,
    pub tenant: Tenant,
    pub agent: Agent,
    pub api_key: String,
    pub router: Router,
}

impl TestContext {
    /// Create a new test context with fresh fixtures.
    ///
    /// Requires TEST_DATABASE_URL environment variable to be set.
    pub async fn new() -> Self {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/nche_test".to_string());

        let db = Database::new(&database_url)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        db.migrate().await.expect("Failed to run migrations");

        let db = Arc::new(db);

        // Create test tenant
        let tenant = db
            .create_tenant(
                &format!("Test Tenant {}", nanoid::nanoid!(8)),
                Some("https://webhook.test/hook"),
                Some("test-webhook-secret"),
                None,
                Some(serde_json::json!(["internal.test.com"])),
            )
            .await
            .expect("Failed to create test tenant");

        // Create test agent with API key
        let (agent, api_key) = db
            .create_agent_with_key(&tenant.id, &format!("Test Agent {}", nanoid::nanoid!(8)))
            .await
            .expect("Failed to create test agent");

        // Create router
        let state = AppState {
            db: db.clone(),
            blocked_email_domains: vec!["blocked.com".to_string()],
        };
        let router = create_router(state);

        Self {
            db,
            tenant,
            agent,
            api_key,
            router,
        }
    }

    /// Create a session for testing.
    pub async fn create_session(&self, autonomy: AutonomyLevel) -> Session {
        self.db
            .create_session(
                &self.tenant.id,
                &self.agent.id,
                "test_actor",
                ActorType::System,
                autonomy,
            )
            .await
            .expect("Failed to create test session")
    }

    /// Create an action for testing.
    pub async fn create_action(
        &self,
        session: &Session,
        tool: &str,
        params: serde_json::Value,
    ) -> Action {
        self.db
            .create_action(&self.tenant.id, &session.id, tool, params)
            .await
            .expect("Failed to create test action")
    }

    /// Make an authenticated API request to the agent API.
    pub async fn agent_request(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(header::CONTENT_TYPE, "application/json");

        let body = match body {
            Some(json) => Body::from(serde_json::to_string(&json).unwrap()),
            None => Body::empty(),
        };

        let request = builder.body(body).unwrap();

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to execute request");

        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body");

        let json: serde_json::Value = if body_bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body_bytes).unwrap_or(serde_json::Value::Null)
        };

        (status, json)
    }

    /// Make an unauthenticated request.
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let body = match body {
            Some(json) => Body::from(serde_json::to_string(&json).unwrap()),
            None => Body::empty(),
        };

        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .unwrap();

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to execute request");

        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body");

        let json: serde_json::Value = if body_bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body_bytes).unwrap_or(serde_json::Value::Null)
        };

        (status, json)
    }

    /// Clean up test data for this tenant.
    pub async fn cleanup(&self) {
        // Delete in order to respect foreign keys
        let _ = sqlx::query("DELETE FROM events WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM webhook_deliveries WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM approvals WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM actions WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM sessions WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM links WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM documents WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM tasks WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM cases WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM agents WHERE tenant_id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;

        let _ = sqlx::query("DELETE FROM tenants WHERE id = $1")
            .bind(&self.tenant.id.0)
            .execute(&self.db.pool)
            .await;
    }
}

/// Create a minimal database context (just DB connection, no fixtures).
#[allow(dead_code)]
pub async fn create_test_db() -> Arc<Database> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/nche_test".to_string());

    let db = Database::new(&database_url)
        .await
        .expect("Failed to connect to test database");

    db.migrate().await.expect("Failed to run migrations");

    Arc::new(db)
}

/// Helper to generate unique test names to avoid collisions.
#[allow(dead_code)]
pub fn unique_name(prefix: &str) -> String {
    format!("{}_{}", prefix, nanoid::nanoid!(8))
}
