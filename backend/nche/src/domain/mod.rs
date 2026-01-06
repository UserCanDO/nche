mod state_machine;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;

// === Identifiers (newtypes for type safety) ===

/// Macro to generate ID wrapper types with a specific prefix.
/// 
/// This macro creates a new struct that wraps a String with the following features:
/// - Generates unique IDs with the format "{prefix}_{nanoid}" where nanoid is 12 characters
/// - Implements common traits: Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type
/// - Transparent SQL type mapping (#[sqlx(transparent)])
/// - Provides new(), from_string(), Default, Display, and AsRef<str> implementations
/// 
/// # Arguments
/// - `$name`: The identifier for the struct to be created
/// - `$prefix`: A string literal prefix to be used in generated IDs
/// 
/// # Example
/// ```rust,ignore
/// define_id!(UserId, "user");
///
/// // Creating a new ID (generates: "user_A1B2C3D4E5F6")
/// let user_id = UserId::new();
///
/// // Creating from an existing string
/// let existing_id = UserId::from_string("user_custom123".to_string());
///
/// // Default implementation (same as new())
/// let default_id: UserId = UserId::default();
///
/// // Display formatting
/// println!("User ID: {}", user_id);
///
/// // String reference access
/// let id_str: &str = user_id.as_ref();
///
/// // Direct access to inner String
/// let inner_string: String = user_id.0;
/// ```
macro_rules! define_id {
    ($name:ident, $prefix:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, nanoid::nanoid!(12)))
            }

            pub fn from_string(s: String) -> Self {
                Self(s)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

define_id!(TenantId, "ten");
define_id!(AgentId, "agt");
define_id!(SessionId, "sess");
define_id!(ActionId, "act");
define_id!(ApprovalId, "appr");
define_id!(EventId, "evt");
define_id!(TaskId, "task");
define_id!(CaseId, "case");
define_id!(DocumentId, "doc");
define_id!(LinkId, "link");
define_id!(WebhookDeliveryId, "whd");
define_id!(DashboardUserId, "user");
define_id!(DashboardSessionId, "dsess");

// === Enums ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    User,
    Org,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    Full,
    Supervised,
    Restricted,
}

/// Action lifecycle states.
///
/// Flow:
/// ```text
/// Proposed -> [policy evaluation]
///          -> ReadyToExecute (if allowed)
///          -> PausedForApproval (if requires approval)
///          -> Denied (if denied by policy)
///
/// PausedForApproval -> [human decision]
///                   -> ReadyToExecute (if approved)
///                   -> Denied (if denied)
///
/// ReadyToExecute -> PendingExecution (webhook sent to tenant)
///
/// PendingExecution -> [tenant executes and reports result]
///                  -> Executed (if success)
///                  -> Failed (if error)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActionState {
    /// Initial state when action is proposed by agent.
    Proposed,
    /// Waiting for human approval.
    PausedForApproval,
    /// Approved and ready - execution webhook will be sent to tenant.
    ReadyToExecute,
    /// Execution webhook sent to tenant, awaiting result.
    PendingExecution,
    /// Tenant reported successful execution.
    Executed,
    /// Denied by policy or human.
    Denied,
    /// Tenant reported execution failure.
    Failed,
}

impl ActionState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Executed | Self::Denied | Self::Failed)
    }

    pub fn is_pending_approval(&self) -> bool {
        matches!(self, Self::PausedForApproval)
    }

    pub fn is_awaiting_execution(&self) -> bool {
        matches!(self, Self::PendingExecution)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PolicyResult {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Escalated,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Action,
    Task,
    Case,
    Document,
    Approval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum WebhookDeliveryStatus {
    Pending,
    Delivered,
    Failed,
}

// === Webhook Events ===

/// Webhook event types sent to tenant's notification webhook.
///
/// Note: `ExecuteAction` is sent to the tenant's *execution* webhook (separate from notification webhook).
/// The other events are sent to the notification webhook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// Action requires human approval.
    ApprovalRequired,
    /// Action was approved by human.
    ActionApproved,
    /// Action was denied by human or policy.
    ActionDenied,
    /// Action was executed successfully by tenant.
    ActionExecuted,
    /// Action execution failed.
    ActionFailed,
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApprovalRequired => write!(f, "approval_required"),
            Self::ActionApproved => write!(f, "action_approved"),
            Self::ActionDenied => write!(f, "action_denied"),
            Self::ActionExecuted => write!(f, "action_executed"),
            Self::ActionFailed => write!(f, "action_failed"),
        }
    }
}

// === Entity Structs ===

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: TenantId,
    pub name: String,
    /// Notification webhook URL (for approval_required, action_executed, etc.).
    pub webhook_url: Option<String>,
    /// Secret for signing notification webhooks.
    pub webhook_secret: Option<String>,
    /// Which notification events to send.
    pub webhook_events: Option<serde_json::Value>,
    /// List of internal email domains for this tenant.
    pub internal_domains: Option<serde_json::Value>,
    /// Execution webhook URL - Nche POSTs here when action is ready to execute.
    /// Tenant executes the tool and reports result back via API.
    pub execution_webhook_url: Option<String>,
    /// Secret for signing execution webhooks.
    pub execution_webhook_secret: Option<String>,
    /// Timeout for execution webhook calls (default 30000ms).
    pub execution_webhook_timeout_ms: Option<i32>,
    /// Policy evaluation mode: "builtin" (default) or "webhook".
    /// When "webhook", policy evaluation is delegated to tenant's policy webhook.
    pub policy_mode: Option<String>,
    /// Policy webhook URL - Nche POSTs here to get policy decision.
    /// Only used when policy_mode = "webhook".
    pub policy_webhook_url: Option<String>,
    /// Secret for signing policy webhooks.
    pub policy_webhook_secret: Option<String>,
    /// Timeout for policy webhook calls (default 500ms - must be fast).
    pub policy_webhook_timeout_ms: Option<i32>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl Tenant {
    /// Returns the list of internal domains for this tenant.
    pub fn get_internal_domains(&self) -> Vec<String> {
        self.internal_domains
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Checks if an email address is to an internal domain.
    pub fn is_internal_email(&self, email: &str) -> bool {
        let internal_domains = self.get_internal_domains();
        if internal_domains.is_empty() {
            return false;
        }

        let email_domain = match email.split('@').last() {
            Some(d) => d.to_lowercase(),
            None => return false,
        };

        internal_domains.iter().any(|domain| {
            let domain = domain.to_lowercase();
            if domain.starts_with("*.") {
                // Wildcard match: *.example.com matches sub.example.com
                let suffix = &domain[1..]; // ".example.com"
                email_domain.ends_with(suffix) || email_domain == &domain[2..]
            } else {
                email_domain == domain
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Agent {
    pub id: AgentId,
    pub tenant_id: TenantId,
    pub name: String,
    #[serde(skip_serializing)]
    pub api_key_hash: String,
    pub api_key_prefix: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: SessionId,
    pub tenant_id: TenantId,
    pub agent_id: AgentId,
    pub actor_id: String,
    pub actor_type: ActorType,
    pub autonomy_level: AutonomyLevel,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Action {
    pub id: ActionId,
    pub tenant_id: TenantId,
    pub session_id: SessionId,
    pub tool: String,
    pub params: serde_json::Value,
    pub state: ActionState,
    pub policy_result: Option<PolicyResult>,
    pub policy_reason: Option<String>,
    /// Legacy field - kept for backwards compatibility.
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    /// Result reported by tenant after execution.
    pub execution_result: Option<serde_json::Value>,
    /// Who/what executed this action (e.g., "tenant_webhook", "manual").
    pub executed_by: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Approval {
    pub id: ApprovalId,
    pub tenant_id: TenantId,
    pub action_id: ActionId,
    pub status: ApprovalStatus,
    pub approver_id: Option<String>,
    pub approver_note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub decided_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub id: EventId,
    pub tenant_id: TenantId,
    pub session_id: Option<SessionId>,
    pub action_id: Option<ActionId>,
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DashboardUser {
    pub id: DashboardUserId,
    pub tenant_id: TenantId,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DashboardSession {
    pub id: DashboardSessionId,
    pub user_id: DashboardUserId,
    pub tenant_id: TenantId,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookDelivery {
    pub id: WebhookDeliveryId,
    pub tenant_id: TenantId,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status: WebhookDeliveryStatus,
    pub attempts: i32,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_attempt_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub next_attempt_at: OffsetDateTime,
    pub last_error: Option<String>,
    pub attempt_metadata: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

// === NCHE-Native Records ===

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: TaskId,
    pub tenant_id: TenantId,
    pub session_id: Option<SessionId>,
    pub title: String,
    pub status: TaskStatus,
    pub notes: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Case {
    pub id: CaseId,
    pub tenant_id: TenantId,
    pub session_id: Option<SessionId>,
    pub title: String,
    pub status: CaseStatus,
    pub severity: Severity,
    pub evidence: serde_json::Value,
    pub external_ref: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Document {
    pub id: DocumentId,
    pub tenant_id: TenantId,
    pub session_id: Option<SessionId>,
    pub doc_type: String,
    pub filename: Option<String>,
    pub checksum: Option<String>,
    pub storage_uri: Option<String>,
    pub tags: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Link {
    pub id: LinkId,
    pub tenant_id: TenantId,
    pub source_type: RecordType,
    pub source_id: String,
    pub target_type: RecordType,
    pub target_id: String,
    pub relation: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ID Type Tests ===

    #[test]
    fn test_tenant_id_generation() {
        let id = TenantId::new();
        assert!(id.0.starts_with("ten_"));
        assert_eq!(id.0.len(), 4 + 12); // "ten_" + 12 char nanoid
    }

    #[test]
    fn test_agent_id_generation() {
        let id = AgentId::new();
        assert!(id.0.starts_with("agt_"));
    }

    #[test]
    fn test_session_id_generation() {
        let id = SessionId::new();
        assert!(id.0.starts_with("sess_"));
    }

    #[test]
    fn test_action_id_generation() {
        let id = ActionId::new();
        assert!(id.0.starts_with("act_"));
    }

    #[test]
    fn test_id_from_string() {
        let custom = "ten_custom123456".to_string();
        let id = TenantId::from_string(custom.clone());
        assert_eq!(id.0, custom);
    }

    #[test]
    fn test_id_display() {
        let id = TenantId::from_string("ten_test123".to_string());
        assert_eq!(format!("{}", id), "ten_test123");
    }

    #[test]
    fn test_id_as_ref() {
        let id = TenantId::from_string("ten_test123".to_string());
        let s: &str = id.as_ref();
        assert_eq!(s, "ten_test123");
    }

    #[test]
    fn test_id_equality() {
        let id1 = TenantId::from_string("ten_test".to_string());
        let id2 = TenantId::from_string("ten_test".to_string());
        let id3 = TenantId::from_string("ten_other".to_string());
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_id_uniqueness() {
        let id1 = TenantId::new();
        let id2 = TenantId::new();
        assert_ne!(id1, id2);
    }

    // === ActionState Tests ===

    #[test]
    fn test_action_state_is_terminal() {
        assert!(!ActionState::Proposed.is_terminal());
        assert!(!ActionState::PausedForApproval.is_terminal());
        assert!(!ActionState::ReadyToExecute.is_terminal());
        assert!(!ActionState::PendingExecution.is_terminal());
        assert!(ActionState::Executed.is_terminal());
        assert!(ActionState::Denied.is_terminal());
        assert!(ActionState::Failed.is_terminal());
    }

    #[test]
    fn test_action_state_is_pending_approval() {
        assert!(!ActionState::Proposed.is_pending_approval());
        assert!(ActionState::PausedForApproval.is_pending_approval());
        assert!(!ActionState::ReadyToExecute.is_pending_approval());
        assert!(!ActionState::PendingExecution.is_pending_approval());
        assert!(!ActionState::Executed.is_pending_approval());
    }

    #[test]
    fn test_action_state_is_awaiting_execution() {
        assert!(!ActionState::Proposed.is_awaiting_execution());
        assert!(!ActionState::PausedForApproval.is_awaiting_execution());
        assert!(!ActionState::ReadyToExecute.is_awaiting_execution());
        assert!(ActionState::PendingExecution.is_awaiting_execution());
        assert!(!ActionState::Executed.is_awaiting_execution());
    }

    // === WebhookEventType Tests ===

    #[test]
    fn test_webhook_event_type_display() {
        assert_eq!(WebhookEventType::ApprovalRequired.to_string(), "approval_required");
        assert_eq!(WebhookEventType::ActionApproved.to_string(), "action_approved");
        assert_eq!(WebhookEventType::ActionDenied.to_string(), "action_denied");
        assert_eq!(WebhookEventType::ActionExecuted.to_string(), "action_executed");
        assert_eq!(WebhookEventType::ActionFailed.to_string(), "action_failed");
    }

    // === Enum Serialization Tests ===

    #[test]
    fn test_autonomy_level_serialization() {
        assert_eq!(
            serde_json::to_string(&AutonomyLevel::Full).unwrap(),
            "\"full\""
        );
        assert_eq!(
            serde_json::to_string(&AutonomyLevel::Supervised).unwrap(),
            "\"supervised\""
        );
        assert_eq!(
            serde_json::to_string(&AutonomyLevel::Restricted).unwrap(),
            "\"restricted\""
        );
    }

    #[test]
    fn test_autonomy_level_deserialization() {
        assert_eq!(
            serde_json::from_str::<AutonomyLevel>("\"full\"").unwrap(),
            AutonomyLevel::Full
        );
        assert_eq!(
            serde_json::from_str::<AutonomyLevel>("\"supervised\"").unwrap(),
            AutonomyLevel::Supervised
        );
    }

    #[test]
    fn test_policy_result_serialization() {
        assert_eq!(
            serde_json::to_string(&PolicyResult::Allow).unwrap(),
            "\"allow\""
        );
        assert_eq!(
            serde_json::to_string(&PolicyResult::Deny).unwrap(),
            "\"deny\""
        );
        assert_eq!(
            serde_json::to_string(&PolicyResult::RequireApproval).unwrap(),
            "\"require_approval\""
        );
    }

    #[test]
    fn test_approval_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ApprovalStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalStatus::Approved).unwrap(),
            "\"approved\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalStatus::Denied).unwrap(),
            "\"denied\""
        );
    }
}
