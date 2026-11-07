//! The frozen job state machine (RUNTIME_MCP_CONTRACT §10). Every state change in the
//! control store goes through [`transition`], so no code path can invent an edge.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Queued,
    Running,
    WaitingRetry,
    NeedsCredentials,
    BlockedPolicy,
    CancelRequested,
    Canceled,
    Succeeded,
    Partial,
    Failed,
}

impl State {
    pub const ALL: [State; 10] = [
        State::Queued,
        State::Running,
        State::WaitingRetry,
        State::NeedsCredentials,
        State::BlockedPolicy,
        State::CancelRequested,
        State::Canceled,
        State::Succeeded,
        State::Partial,
        State::Failed,
    ];

    pub fn name(self) -> &'static str {
        match self {
            State::Queued => "queued",
            State::Running => "running",
            State::WaitingRetry => "waiting_retry",
            State::NeedsCredentials => "needs_credentials",
            State::BlockedPolicy => "blocked_policy",
            State::CancelRequested => "cancel_requested",
            State::Canceled => "canceled",
            State::Succeeded => "succeeded",
            State::Partial => "partial",
            State::Failed => "failed",
        }
    }

    pub fn parse(name: &str) -> Option<State> {
        State::ALL.into_iter().find(|s| s.name() == name)
    }

    pub fn terminal(self) -> bool {
        matches!(
            self,
            State::Canceled | State::Succeeded | State::Partial | State::Failed
        )
    }
}

/// Why a state changes (contract §10's trigger column).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Worker claim after recovery and ownership checks, grant rechecked.
    Claim,
    /// Token-endpoint 429/5xx/transport failure, within cooldown and deadline.
    TransientFailure,
    /// Second 401 after the single replay, or a permanent 403/denial.
    CredentialDenied,
    /// Grant revoked, quota exhausted, quarantine, low space.
    PolicyBlock,
    /// `jobs_cancel`.
    CancelRequest,
    /// The next checkpoint observes the request (no new sends, publication or
    /// destructive steps).
    CancelCheckpoint,
    /// Terminal outcomes of a running job.
    Succeed,
    PartialResult,
    Fail,
    /// A validated same-scope credential revision.
    CredentialsRestored,
    /// The retry time is reached within caps.
    RetryDue,
    /// The blocking condition cleared, rechecked at claim time.
    PolicyCleared,
    /// The job's original deadline passed.
    DeadlinePassed,
}

/// The single source of truth for allowed edges. `None` means refused: the caller keeps
/// the current state and reports a conflict.
pub fn transition(from: State, trigger: Trigger) -> Option<State> {
    use State::*;
    use Trigger::*;
    match (from, trigger) {
        (Queued, Claim) => Some(Running),
        (Running, TransientFailure) => Some(WaitingRetry),
        (Running, CredentialDenied) => Some(NeedsCredentials),
        (Running, PolicyBlock) => Some(BlockedPolicy),
        (Running | Queued | WaitingRetry | NeedsCredentials | BlockedPolicy, CancelRequest) => {
            Some(CancelRequested)
        }
        (CancelRequested, CancelCheckpoint) => Some(Canceled),
        (Running, Succeed) => Some(Succeeded),
        (Running, PartialResult) => Some(Partial),
        (Running, Fail) => Some(Failed),
        (NeedsCredentials, CredentialsRestored) => Some(Queued),
        (WaitingRetry, RetryDue) => Some(Queued),
        (BlockedPolicy, PolicyCleared) => Some(Queued),
        (NeedsCredentials | BlockedPolicy | WaitingRetry, DeadlinePassed) => Some(Failed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_admit_no_edge() {
        let triggers = [
            Trigger::Claim,
            Trigger::TransientFailure,
            Trigger::CredentialDenied,
            Trigger::PolicyBlock,
            Trigger::CancelRequest,
            Trigger::CancelCheckpoint,
            Trigger::Succeed,
            Trigger::PartialResult,
            Trigger::Fail,
            Trigger::CredentialsRestored,
            Trigger::RetryDue,
            Trigger::PolicyCleared,
            Trigger::DeadlinePassed,
        ];
        for state in State::ALL.into_iter().filter(|s| s.terminal()) {
            for trigger in triggers {
                assert_eq!(transition(state, trigger), None, "{state:?} {trigger:?}");
            }
        }
        // Exactly the contract's edges: 19 (from, trigger) pairs admit a change.
        let edges = State::ALL
            .iter()
            .flat_map(|s| {
                triggers
                    .iter()
                    .filter(move |t| transition(*s, **t).is_some())
            })
            .count();
        assert_eq!(edges, 19);
    }

    #[test]
    fn cancellation_completes_only_at_a_checkpoint_and_never_from_terminal() {
        assert_eq!(
            transition(State::Running, Trigger::CancelRequest),
            Some(State::CancelRequested)
        );
        assert_eq!(transition(State::Running, Trigger::CancelCheckpoint), None);
        assert_eq!(
            transition(State::CancelRequested, Trigger::CancelCheckpoint),
            Some(State::Canceled)
        );
        assert_eq!(transition(State::CancelRequested, Trigger::Succeed), None);
        assert_eq!(transition(State::Succeeded, Trigger::CancelRequest), None);
    }

    #[test]
    fn names_round_trip() {
        for s in State::ALL {
            assert_eq!(State::parse(s.name()), Some(s));
        }
        assert_eq!(State::parse("done"), None);
    }
}
