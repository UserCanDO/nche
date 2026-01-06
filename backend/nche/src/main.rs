use std::net::SocketAddr;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use nche::api::{create_router, AppState};
use nche::config::NcheConfig;
use nche::db::Database;
use nche::domain::{ApprovalStatus, TenantId};
use nche::{spawn_executor, spawn_webhook_dispatcher, ExecutorConfig, WebhookDispatcherConfig};

#[derive(Parser)]
#[command(name = "nche")]
#[command(about = "Agent Control Plane - Multi-tenant infrastructure for AI agent oversight")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server
    Serve {
        /// Host to bind to
        #[arg(long, env = "SERVER_HOST", default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(long, env = "SERVER_PORT", default_value = "3000")]
        port: u16,

        /// Disable the background action executor
        #[arg(long, env = "EXECUTOR_DISABLED", default_value = "false")]
        executor_disabled: bool,

        /// Executor poll interval in milliseconds
        #[arg(long, env = "EXECUTOR_POLL_INTERVAL_MS", default_value = "1000")]
        executor_poll_interval_ms: u64,

        /// Executor batch size (max actions per poll)
        #[arg(long, env = "EXECUTOR_BATCH_SIZE", default_value = "10")]
        executor_batch_size: i64,

        /// Disable the webhook dispatcher
        #[arg(long, env = "WEBHOOK_DISPATCHER_DISABLED", default_value = "false")]
        webhook_disabled: bool,

        /// Webhook dispatcher poll interval in milliseconds
        #[arg(long, env = "WEBHOOK_POLL_INTERVAL_MS", default_value = "5000")]
        webhook_poll_interval_ms: u64,

        /// Webhook dispatcher batch size
        #[arg(long, env = "WEBHOOK_BATCH_SIZE", default_value = "20")]
        webhook_batch_size: i64,

        /// Webhook max retries
        #[arg(long, env = "WEBHOOK_MAX_RETRIES", default_value = "5")]
        webhook_max_retries: i32,
    },

    /// Run database migrations
    Migrate,

    /// Initialize the system (migrate + create bootstrap tenant/agent/user)
    Init {
        /// Name for the bootstrap tenant
        #[arg(long, default_value = "Default Tenant")]
        tenant_name: String,

        /// Name for the bootstrap agent
        #[arg(long, default_value = "Default Agent")]
        agent_name: String,

        /// Email for the bootstrap dashboard user
        #[arg(long, default_value = "admin@localhost")]
        user_email: String,

        /// Password for the bootstrap dashboard user
        #[arg(long, default_value = "admin")]
        user_password: String,

        /// Optional webhook URL for the tenant
        #[arg(long)]
        webhook_url: Option<String>,
    },

    /// Tenant management commands
    Tenants {
        #[command(subcommand)]
        command: TenantsCommands,
    },

    /// Approval management commands
    Approvals {
        #[command(subcommand)]
        command: ApprovalsCommands,
    },
}

#[derive(Subcommand)]
enum TenantsCommands {
    /// List all tenants
    List {
        /// Maximum number of tenants to show
        #[arg(long, default_value = "100")]
        limit: i64,
    },

    /// Create a new tenant
    Create {
        /// Tenant name
        #[arg(long)]
        name: String,

        /// Webhook URL for notifications
        #[arg(long)]
        webhook_url: Option<String>,

        /// Webhook secret for HMAC signing
        #[arg(long)]
        webhook_secret: Option<String>,
    },
}

#[derive(Subcommand)]
enum ApprovalsCommands {
    /// List approvals
    List {
        /// Filter by tenant ID
        #[arg(long)]
        tenant: Option<String>,

        /// Filter by status (pending, approved, denied)
        #[arg(long, default_value = "pending")]
        status: String,

        /// Maximum number of approvals to show
        #[arg(long, default_value = "50")]
        limit: i64,
    },

    /// Approve an action
    Approve {
        /// Approval ID
        id: String,

        /// Approver identifier (e.g., email or username)
        #[arg(long)]
        approver: String,

        /// Optional note
        #[arg(long)]
        note: Option<String>,
    },

    /// Deny an action
    Deny {
        /// Approval ID
        id: String,

        /// Approver identifier (e.g., email or username)
        #[arg(long)]
        approver: String,

        /// Reason for denial (required)
        #[arg(long)]
        reason: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file first
    dotenvy::dotenv().ok();

    // Load config from nche.yaml (if present) and apply env overrides
    // This sets env vars from config file before clap parses them
    let config = NcheConfig::load();

    // Set env vars from config if not already set (for clap to pick up)
    set_env_if_unset("DATABASE_URL", config.database.url.as_deref());
    set_env_if_unset("SERVER_HOST", Some(&config.server.host));
    set_env_if_unset("SERVER_PORT", Some(&config.server.port.to_string()));
    set_env_if_unset("EXECUTOR_DISABLED", Some(&config.executor.disabled.to_string()));
    set_env_if_unset("EXECUTOR_POLL_INTERVAL_MS", Some(&config.executor.poll_interval_ms.to_string()));
    set_env_if_unset("EXECUTOR_BATCH_SIZE", Some(&config.executor.batch_size.to_string()));
    set_env_if_unset("WEBHOOK_DISPATCHER_DISABLED", Some(&config.webhooks.disabled.to_string()));
    set_env_if_unset("WEBHOOK_POLL_INTERVAL_MS", Some(&config.webhooks.poll_interval_ms.to_string()));
    set_env_if_unset("WEBHOOK_BATCH_SIZE", Some(&config.webhooks.batch_size.to_string()));
    set_env_if_unset("WEBHOOK_MAX_RETRIES", Some(&config.webhooks.max_retries.to_string()));

    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            config.logging.level.clone().into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment or nche.yaml");

    let db = Database::new(&database_url).await?;

    match cli.command {
        Commands::Serve {
            host,
            port,
            executor_disabled,
            executor_poll_interval_ms,
            executor_batch_size,
            webhook_disabled,
            webhook_poll_interval_ms,
            webhook_batch_size,
            webhook_max_retries,
        } => {
            // Run migrations on startup
            tracing::info!("Running database migrations...");
            db.migrate().await?;

            let db = Arc::new(db);

            // Spawn the action executor
            let executor_config = ExecutorConfig {
                poll_interval_ms: executor_poll_interval_ms,
                batch_size: executor_batch_size,
                enabled: !executor_disabled,
            };
            let _executor_handle = spawn_executor(db.clone(), executor_config);

            // Spawn the webhook dispatcher
            let webhook_config = WebhookDispatcherConfig {
                poll_interval_ms: webhook_poll_interval_ms,
                batch_size: webhook_batch_size,
                max_retries: webhook_max_retries,
                enabled: !webhook_disabled,
                ..Default::default()
            };
            let _webhook_handle = spawn_webhook_dispatcher(db.clone(), webhook_config);

            let state = AppState {
                db,
                blocked_email_domains: config.policy.blocked_email_domains.clone(),
            };
            let app = create_router(state);

            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            tracing::info!("Starting server on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }

        Commands::Migrate => {
            tracing::info!("Running database migrations...");
            db.migrate().await?;
            println!("✓ Migrations complete");
        }

        Commands::Init {
            tenant_name,
            agent_name,
            user_email,
            user_password,
            webhook_url,
        } => {
            println!("Initializing NCHE...\n");

            // Run migrations
            println!("Running migrations...");
            db.migrate().await?;
            println!("✓ Migrations complete\n");

            // Create tenant
            println!("Creating tenant: {}", tenant_name);
            let tenant = db
                .create_tenant(&tenant_name, webhook_url.as_deref(), None, None, None)
                .await?;
            println!("✓ Tenant created: {}\n", tenant.id);

            // Create agent
            println!("Creating agent: {}", agent_name);
            let (agent, api_key) = db.create_agent_with_key(&tenant.id, &agent_name).await?;
            println!("✓ Agent created: {}", agent.id);
            println!("  API Key: {}\n", api_key);
            println!("  ⚠️  Save this API key - it cannot be retrieved later!\n");

            // Create dashboard user
            println!("Creating dashboard user: {}", user_email);
            let user = db
                .create_dashboard_user(&tenant.id, &user_email, &user_password, Some(&tenant_name))
                .await?;
            println!("✓ Dashboard user created: {}\n", user.id);

            println!("═══════════════════════════════════════════════════════════");
            println!("NCHE initialized successfully!");
            println!("═══════════════════════════════════════════════════════════");
            println!();
            println!("Tenant ID:    {}", tenant.id);
            println!("Agent ID:     {}", agent.id);
            println!("API Key:      {}", api_key);
            println!();
            println!("Dashboard:    http://localhost:3000/");
            println!("Login:        {} / {}", user_email, user_password);
            println!();
            println!("Start the server with: nche serve");
            println!("═══════════════════════════════════════════════════════════");
        }

        Commands::Tenants { command } => match command {
            TenantsCommands::List { limit } => {
                let tenants = db.list_tenants(limit).await?;

                if tenants.is_empty() {
                    println!("No tenants found.");
                    return Ok(());
                }

                println!(
                    "{:<20} {:<30} {:<40} {}",
                    "ID", "NAME", "WEBHOOK URL", "CREATED"
                );
                println!("{}", "-".repeat(110));

                for tenant in tenants {
                    let webhook = tenant
                        .webhook_url
                        .as_deref()
                        .unwrap_or("-")
                        .chars()
                        .take(38)
                        .collect::<String>();
                    let created = tenant
                        .created_at
                        .format(&time::format_description::well_known::Rfc3339)?;
                    println!(
                        "{:<20} {:<30} {:<40} {}",
                        tenant.id.0,
                        truncate(&tenant.name, 28),
                        webhook,
                        created
                    );
                }
            }

            TenantsCommands::Create {
                name,
                webhook_url,
                webhook_secret,
            } => {
                let tenant = db
                    .create_tenant(&name, webhook_url.as_deref(), webhook_secret.as_deref(), None, None)
                    .await?;

                println!("✓ Tenant created successfully!\n");
                println!("  ID:          {}", tenant.id);
                println!("  Name:        {}", tenant.name);
                if let Some(url) = &tenant.webhook_url {
                    println!("  Webhook URL: {}", url);
                }
            }
        },

        Commands::Approvals { command } => match command {
            ApprovalsCommands::List {
                tenant,
                status,
                limit,
            } => {
                let status_filter = match status.to_lowercase().as_str() {
                    "pending" => Some(ApprovalStatus::Pending),
                    "approved" => Some(ApprovalStatus::Approved),
                    "denied" => Some(ApprovalStatus::Denied),
                    "all" => None,
                    _ => {
                        eprintln!(
                            "Invalid status: {}. Use: pending, approved, denied, or all",
                            status
                        );
                        std::process::exit(1);
                    }
                };

                let tenant_id = tenant.map(TenantId::from_string);

                let approvals = db
                    .list_all_approvals(tenant_id.as_ref(), status_filter, limit)
                    .await?;

                if approvals.is_empty() {
                    println!("No approvals found.");
                    return Ok(());
                }

                println!(
                    "{:<20} {:<20} {:<12} {:<20} {}",
                    "APPROVAL ID", "ACTION ID", "STATUS", "TENANT", "CREATED"
                );
                println!("{}", "-".repeat(100));

                for approval in approvals {
                    let created = approval
                        .created_at
                        .format(&time::format_description::well_known::Rfc3339)?;
                    let status_str = match approval.status {
                        ApprovalStatus::Pending => "pending",
                        ApprovalStatus::Approved => "approved",
                        ApprovalStatus::Denied => "denied",
                    };
                    println!(
                        "{:<20} {:<20} {:<12} {:<20} {}",
                        approval.id.0, approval.action_id.0, status_str, approval.tenant_id.0, created
                    );
                }
            }

            ApprovalsCommands::Approve { id, approver, note } => {
                let approval_id = nche::domain::ApprovalId::from_string(id.clone());

                // Get the approval by ID
                let approval = db.get_approval_by_id(&approval_id).await?;

                match approval {
                    Some(approval) if approval.status == ApprovalStatus::Pending => {
                        db.decide_approval(
                            &approval.tenant_id,
                            &approval_id,
                            true,
                            &approver,
                            note.as_deref(),
                        )
                        .await?;
                        println!("✓ Approval {} approved by {}", id, approver);
                    }
                    Some(_) => {
                        eprintln!("Approval {} is not pending", id);
                        std::process::exit(1);
                    }
                    None => {
                        eprintln!("Approval {} not found", id);
                        std::process::exit(1);
                    }
                }
            }

            ApprovalsCommands::Deny {
                id,
                approver,
                reason,
            } => {
                let approval_id = nche::domain::ApprovalId::from_string(id.clone());

                // Get the approval by ID
                let approval = db.get_approval_by_id(&approval_id).await?;

                match approval {
                    Some(approval) if approval.status == ApprovalStatus::Pending => {
                        db.decide_approval(
                            &approval.tenant_id,
                            &approval_id,
                            false,
                            &approver,
                            Some(&reason),
                        )
                        .await?;
                        println!("✓ Approval {} denied by {}: {}", id, approver, reason);
                    }
                    Some(_) => {
                        eprintln!("Approval {} is not pending", id);
                        std::process::exit(1);
                    }
                    None => {
                        eprintln!("Approval {} not found", id);
                        std::process::exit(1);
                    }
                }
            }
        },
    }

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Set environment variable if not already set
/// SAFETY: Called early in main before any threads are spawned
fn set_env_if_unset(key: &str, value: Option<&str>) {
    if std::env::var(key).is_err() {
        if let Some(v) = value {
            // SAFETY: This is called at startup before any threads are spawned
            unsafe { std::env::set_var(key, v) };
        }
    }
}
