mod actions;
mod agents;
mod approvals;
mod cases;
mod dashboard;
mod documents;
mod events;
mod links;
mod sessions;
mod tasks;
mod tenants;
mod webhooks;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::error::{NcheError, Result};

pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(NcheError::Database)?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| NcheError::Internal(format!("Migration failed: {}", e)))?;

        Ok(())
    }
}
