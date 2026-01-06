//! Configuration file loading for NCHE
//!
//! Supports loading configuration from:
//! 1. nche.yaml (or nche.yml) in current directory
//! 2. Environment variables (take precedence over file)
//!
//! Example nche.yaml:
//! ```yaml
//! database:
//!   url: postgres://user:pass@localhost:5432/nche
//!
//! server:
//!   host: 127.0.0.1
//!   port: 3000
//!
//! executor:
//!   disabled: false
//!   poll_interval_ms: 1000
//!   batch_size: 10
//!
//! webhooks:
//!   disabled: false
//!   poll_interval_ms: 5000
//!   batch_size: 20
//!   max_retries: 5
//!
//! # Default modes for new tenants
//! defaults:
//!   execution_mode: webhook  # or "builtin" for self-hosted
//!   policy_mode: builtin     # or "webhook" for custom policies
//!
//! policy:
//!   blocked_email_domains:
//!     - competitor.com
//! ```

use serde::Deserialize;
use std::path::Path;

/// Root configuration structure
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct NcheConfig {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub executor: ExecutorConfig,
    pub webhooks: WebhookConfig,
    pub logging: LoggingConfig,
    pub defaults: DefaultsConfig,
    pub policy: PolicyConfig,
}

/// Default modes for new tenants
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    /// Default execution mode for new tenants: "webhook" or "builtin"
    /// - webhook: Tenant receives execution webhooks and runs tools themselves
    /// - builtin: NCHE executes tools directly (self-hosted mode)
    pub execution_mode: String,
    /// Default policy mode for new tenants: "builtin" or "webhook"
    /// - builtin: Use NCHE's built-in 20 semantic tool policies
    /// - webhook: Delegate policy decisions to tenant's webhook
    pub policy_mode: String,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            execution_mode: "webhook".to_string(),
            policy_mode: "builtin".to_string(),
        }
    }
}

/// Policy configuration for controlling agent behavior
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    /// List of blocked email domains. Emails to these domains are always denied.
    /// Supports wildcards: "*.gov" blocks all .gov domains.
    pub blocked_email_domains: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ExecutorConfig {
    pub disabled: bool,
    pub poll_interval_ms: u64,
    pub batch_size: i64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            poll_interval_ms: 1000,
            batch_size: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WebhookConfig {
    pub disabled: bool,
    pub poll_interval_ms: u64,
    pub batch_size: i64,
    pub max_retries: i32,
    pub base_retry_delay_secs: i64,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            poll_interval_ms: 5000,
            batch_size: 20,
            max_retries: 5,
            base_retry_delay_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

impl NcheConfig {
    /// Load configuration from file and environment variables.
    /// Environment variables take precedence over file values.
    pub fn load() -> Self {
        let mut config = Self::load_from_file().unwrap_or_default();
        config.apply_env_overrides();
        config
    }

    /// Load configuration from nche.yaml or nche.yml
    fn load_from_file() -> Option<Self> {
        let paths = ["nche.yaml", "nche.yml"];

        for path in paths {
            if Path::new(path).exists() {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        match serde_yaml::from_str(&content) {
                            Ok(config) => {
                                tracing::info!("Loaded configuration from {}", path);
                                return Some(config);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse {}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read {}: {}", path, e);
                    }
                }
            }
        }

        None
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // Database
        if let Ok(url) = std::env::var("DATABASE_URL") {
            self.database.url = Some(url);
        }

        // Server
        if let Ok(host) = std::env::var("SERVER_HOST") {
            self.server.host = host;
        }
        if let Ok(port) = std::env::var("SERVER_PORT") {
            if let Ok(p) = port.parse() {
                self.server.port = p;
            }
        }

        // Executor
        if let Ok(disabled) = std::env::var("EXECUTOR_DISABLED") {
            self.executor.disabled = disabled.parse().unwrap_or(false);
        }
        if let Ok(interval) = std::env::var("EXECUTOR_POLL_INTERVAL_MS") {
            if let Ok(i) = interval.parse() {
                self.executor.poll_interval_ms = i;
            }
        }
        if let Ok(batch) = std::env::var("EXECUTOR_BATCH_SIZE") {
            if let Ok(b) = batch.parse() {
                self.executor.batch_size = b;
            }
        }

        // Webhooks
        if let Ok(disabled) = std::env::var("WEBHOOK_DISPATCHER_DISABLED") {
            self.webhooks.disabled = disabled.parse().unwrap_or(false);
        }
        if let Ok(interval) = std::env::var("WEBHOOK_POLL_INTERVAL_MS") {
            if let Ok(i) = interval.parse() {
                self.webhooks.poll_interval_ms = i;
            }
        }
        if let Ok(batch) = std::env::var("WEBHOOK_BATCH_SIZE") {
            if let Ok(b) = batch.parse() {
                self.webhooks.batch_size = b;
            }
        }
        if let Ok(retries) = std::env::var("WEBHOOK_MAX_RETRIES") {
            if let Ok(r) = retries.parse() {
                self.webhooks.max_retries = r;
            }
        }

        // Logging
        if let Ok(level) = std::env::var("RUST_LOG") {
            self.logging.level = level;
        }

        // Defaults for new tenants
        if let Ok(mode) = std::env::var("DEFAULT_EXECUTION_MODE") {
            self.defaults.execution_mode = mode;
        }
        if let Ok(mode) = std::env::var("DEFAULT_POLICY_MODE") {
            self.defaults.policy_mode = mode;
        }
    }

    /// Get database URL, panics if not configured
    pub fn database_url(&self) -> &str {
        self.database.url.as_ref().expect("DATABASE_URL must be set in environment or config file")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NcheConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert!(!config.executor.disabled);
        assert_eq!(config.executor.poll_interval_ms, 1000);
        assert_eq!(config.executor.batch_size, 10);
        assert!(!config.webhooks.disabled);
        assert_eq!(config.webhooks.poll_interval_ms, 5000);
        assert_eq!(config.webhooks.batch_size, 20);
        assert_eq!(config.webhooks.max_retries, 5);
        // Defaults for new tenants
        assert_eq!(config.defaults.execution_mode, "webhook");
        assert_eq!(config.defaults.policy_mode, "builtin");
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
database:
  url: postgres://test:test@localhost/testdb

server:
  host: 0.0.0.0
  port: 8080

executor:
  disabled: true
  poll_interval_ms: 2000
  batch_size: 5

webhooks:
  disabled: true
  poll_interval_ms: 10000
  batch_size: 50
  max_retries: 3
"#;

        let config: NcheConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.database.url, Some("postgres://test:test@localhost/testdb".to_string()));
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert!(config.executor.disabled);
        assert_eq!(config.executor.poll_interval_ms, 2000);
        assert_eq!(config.executor.batch_size, 5);
        assert!(config.webhooks.disabled);
        assert_eq!(config.webhooks.poll_interval_ms, 10000);
        assert_eq!(config.webhooks.batch_size, 50);
        assert_eq!(config.webhooks.max_retries, 3);
    }

    #[test]
    fn test_partial_yaml() {
        let yaml = r#"
server:
  port: 9000
"#;

        let config: NcheConfig = serde_yaml::from_str(yaml).unwrap();

        // Specified value
        assert_eq!(config.server.port, 9000);

        // Defaults for unspecified values
        assert_eq!(config.server.host, "127.0.0.1");
        assert!(!config.executor.disabled);
        assert_eq!(config.executor.poll_interval_ms, 1000);
    }

    #[test]
    fn test_env_override() {
        let mut config = NcheConfig::default();
        config.server.port = 3000;

        // Simulate env var
        // SAFETY: Test is single-threaded
        unsafe { std::env::set_var("SERVER_PORT", "4000") };
        config.apply_env_overrides();

        assert_eq!(config.server.port, 4000);

        // Clean up
        // SAFETY: Test is single-threaded
        unsafe { std::env::remove_var("SERVER_PORT") };
    }

    #[test]
    fn test_parse_yaml_with_defaults() {
        let yaml = r#"
defaults:
  execution_mode: builtin
  policy_mode: webhook
"#;

        let config: NcheConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.defaults.execution_mode, "builtin");
        assert_eq!(config.defaults.policy_mode, "webhook");
    }

    #[test]
    fn test_defaults_env_override() {
        let mut config = NcheConfig::default();

        // SAFETY: Test is single-threaded
        unsafe {
            std::env::set_var("DEFAULT_EXECUTION_MODE", "builtin");
            std::env::set_var("DEFAULT_POLICY_MODE", "webhook");
        };
        config.apply_env_overrides();

        assert_eq!(config.defaults.execution_mode, "builtin");
        assert_eq!(config.defaults.policy_mode, "webhook");

        // Clean up
        // SAFETY: Test is single-threaded
        unsafe {
            std::env::remove_var("DEFAULT_EXECUTION_MODE");
            std::env::remove_var("DEFAULT_POLICY_MODE");
        };
    }
}
