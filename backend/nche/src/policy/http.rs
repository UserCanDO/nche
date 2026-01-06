use crate::domain::{Action, AutonomyLevel, Session};

use super::PolicyDecision;

pub fn evaluate(session: &Session, action: &Action) -> PolicyDecision {
    match session.autonomy_level {
        AutonomyLevel::Full => PolicyDecision::allow("Full autonomy - HTTP allowed"),
        AutonomyLevel::Restricted => {
            PolicyDecision::require_approval("HTTP requires approval in restricted mode")
        }
        AutonomyLevel::Supervised => {
            // In supervised mode: GET requests are allowed, anything else requires approval
            if is_safe_method(action) {
                PolicyDecision::allow("GET request allowed in supervised mode")
            } else {
                PolicyDecision::require_approval("Non-GET HTTP request requires approval in supervised mode")
            }
        }
    }
}

fn is_safe_method(action: &Action) -> bool {
    action
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .is_some_and(|m| m.eq_ignore_ascii_case("GET"))
}
