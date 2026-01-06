use crate::domain::{Action, AutonomyLevel, Session};

use super::{PolicyContext, PolicyDecision};

/// Extract the email domain from an email address.
fn extract_email_domain(email: &str) -> Option<&str> {
    email.split('@').last()
}

/// Check if a domain matches a pattern (supports wildcards like *.gov).
fn domain_matches_pattern(domain: &str, pattern: &str) -> bool {
    let domain = domain.to_lowercase();
    let pattern = pattern.to_lowercase();

    if pattern.starts_with("*.") {
        // Wildcard match: *.gov matches example.gov and sub.example.gov
        let suffix = &pattern[1..]; // ".gov"
        domain.ends_with(suffix) || domain == &pattern[2..]
    } else {
        domain == pattern
    }
}

/// Check if an email is to a blocked domain.
fn is_blocked_email(email: &str, blocked_domains: &[String]) -> Option<String> {
    let domain = extract_email_domain(email)?;

    for blocked in blocked_domains {
        if domain_matches_pattern(domain, blocked) {
            return Some(blocked.clone());
        }
    }
    None
}

pub fn evaluate(session: &Session, action: &Action, ctx: &PolicyContext) -> PolicyDecision {
    // Extract email recipient from params
    let to_email = action
        .params
        .get("to")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Step 1: Check blocked domains (always denied)
    if let Some(blocked_domain) = is_blocked_email(to_email, ctx.blocked_email_domains) {
        return PolicyDecision::deny(format!(
            "Email to blocked domain '{}' is not permitted",
            blocked_domain
        ));
    }

    // Step 2: Check autonomy level with internal domain consideration
    match session.autonomy_level {
        AutonomyLevel::Full => PolicyDecision::allow("Full autonomy - email allowed"),

        AutonomyLevel::Supervised => {
            // Check if email is to an internal domain
            if let Some(tenant) = ctx.tenant {
                if tenant.is_internal_email(to_email) {
                    return PolicyDecision::allow(
                        "Email to internal domain auto-approved in supervised mode",
                    );
                }
            }
            PolicyDecision::require_approval("External email requires approval in supervised mode")
        }

        AutonomyLevel::Restricted => {
            PolicyDecision::require_approval("Email requires approval in restricted mode")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_email_domain() {
        assert_eq!(extract_email_domain("user@example.com"), Some("example.com"));
        assert_eq!(extract_email_domain("user@sub.example.com"), Some("sub.example.com"));
        assert_eq!(extract_email_domain("invalid"), Some("invalid"));
        assert_eq!(extract_email_domain(""), Some(""));
    }

    #[test]
    fn test_domain_matches_pattern_exact() {
        assert!(domain_matches_pattern("example.com", "example.com"));
        assert!(domain_matches_pattern("EXAMPLE.COM", "example.com"));
        assert!(!domain_matches_pattern("sub.example.com", "example.com"));
    }

    #[test]
    fn test_domain_matches_pattern_wildcard() {
        assert!(domain_matches_pattern("example.gov", "*.gov"));
        assert!(domain_matches_pattern("sub.example.gov", "*.gov"));
        assert!(domain_matches_pattern("EXAMPLE.GOV", "*.gov"));
        assert!(!domain_matches_pattern("example.com", "*.gov"));
    }

    #[test]
    fn test_is_blocked_email() {
        let blocked = vec!["competitor.com".to_string(), "*.gov".to_string()];

        assert_eq!(
            is_blocked_email("user@competitor.com", &blocked),
            Some("competitor.com".to_string())
        );
        assert_eq!(
            is_blocked_email("user@agency.gov", &blocked),
            Some("*.gov".to_string())
        );
        assert_eq!(is_blocked_email("user@safe.com", &blocked), None);
    }
}
