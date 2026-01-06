//! Action state machine transitions.
//!
//! Defines valid state transitions for actions in the Nche governance flow.
//!
//! Flow:
//! ```text
//! Proposed -> [policy] -> ReadyToExecute | PausedForApproval | Denied
//! PausedForApproval -> [approval] -> ReadyToExecute | Denied
//! ReadyToExecute -> [webhook sent] -> PendingExecution
//! PendingExecution -> [tenant reports] -> Executed | Failed
//! ```

use crate::domain::{ActionState, PolicyResult};
use crate::error::{NcheError, Result};

impl ActionState {
    /// Apply policy evaluation result to a proposed action.
    pub fn apply_policy(self, result: PolicyResult) -> Result<Self> {
        match (self, result) {
            (Self::Proposed, PolicyResult::Allow) => Ok(Self::ReadyToExecute),
            (Self::Proposed, PolicyResult::Deny) => Ok(Self::Denied),
            (Self::Proposed, PolicyResult::RequireApproval) => Ok(Self::PausedForApproval),
            _ => Err(NcheError::InvalidStateTransition {
                from: self,
                action: "apply_policy".into(),
            }),
        }
    }

    /// Apply human approval/denial decision.
    pub fn apply_approval(self, approved: bool) -> Result<Self> {
        match (self, approved) {
            (Self::PausedForApproval, true) => Ok(Self::ReadyToExecute),
            (Self::PausedForApproval, false) => Ok(Self::Denied),
            _ => Err(NcheError::InvalidStateTransition {
                from: self,
                action: "apply_approval".into(),
            }),
        }
    }

    /// Mark action as sent to tenant for execution.
    /// Called when execution webhook is dispatched to tenant.
    pub fn send_for_execution(self) -> Result<Self> {
        match self {
            Self::ReadyToExecute => Ok(Self::PendingExecution),
            _ => Err(NcheError::InvalidStateTransition {
                from: self,
                action: "send_for_execution".into(),
            }),
        }
    }

    /// Record execution result from tenant.
    /// Called when tenant reports back via POST /v1/actions/:id/result.
    pub fn record_execution_result(self, success: bool) -> Result<Self> {
        match (self, success) {
            (Self::PendingExecution, true) => Ok(Self::Executed),
            (Self::PendingExecution, false) => Ok(Self::Failed),
            _ => Err(NcheError::InvalidStateTransition {
                from: self,
                action: "record_execution_result".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_allow_flow() {
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::Allow).unwrap();
        assert_eq!(state, ActionState::ReadyToExecute);
    }

    #[test]
    fn test_policy_deny_flow() {
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::Deny).unwrap();
        assert_eq!(state, ActionState::Denied);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_approval_flow() {
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::RequireApproval).unwrap();
        assert_eq!(state, ActionState::PausedForApproval);

        // Approve
        let state = state.apply_approval(true).unwrap();
        assert_eq!(state, ActionState::ReadyToExecute);
    }

    #[test]
    fn test_denial_flow() {
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::RequireApproval).unwrap();
        assert_eq!(state, ActionState::PausedForApproval);

        // Deny
        let state = state.apply_approval(false).unwrap();
        assert_eq!(state, ActionState::Denied);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_execution_flow() {
        // ReadyToExecute -> PendingExecution
        let state = ActionState::ReadyToExecute;
        let state = state.send_for_execution().unwrap();
        assert_eq!(state, ActionState::PendingExecution);
        assert!(state.is_awaiting_execution());

        // PendingExecution -> Executed
        let state = state.record_execution_result(true).unwrap();
        assert_eq!(state, ActionState::Executed);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_execution_failure_flow() {
        let state = ActionState::PendingExecution;
        let state = state.record_execution_result(false).unwrap();
        assert_eq!(state, ActionState::Failed);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_full_approved_flow() {
        // Proposed -> RequireApproval -> PausedForApproval
        let state = ActionState::Proposed;
        let state = state.apply_policy(PolicyResult::RequireApproval).unwrap();
        assert_eq!(state, ActionState::PausedForApproval);

        // Approve -> ReadyToExecute
        let state = state.apply_approval(true).unwrap();
        assert_eq!(state, ActionState::ReadyToExecute);

        // Send to tenant -> PendingExecution
        let state = state.send_for_execution().unwrap();
        assert_eq!(state, ActionState::PendingExecution);

        // Tenant reports success -> Executed
        let state = state.record_execution_result(true).unwrap();
        assert_eq!(state, ActionState::Executed);
    }

    #[test]
    fn test_invalid_transition() {
        // Can't send executed action for execution
        let state = ActionState::Executed;
        let result = state.send_for_execution();
        assert!(result.is_err());

        // Can't record result for non-pending action
        let state = ActionState::ReadyToExecute;
        let result = state.record_execution_result(true);
        assert!(result.is_err());
    }
}
