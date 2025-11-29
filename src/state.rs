//! Application state machine
//!
//! Defines the AppState enum and state transition logic.

/// Application states following the ReAct pattern
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    /// User typing initial query
    Input,

    /// Waiting for AI response (show spinner)
    Thinking,

    /// AI proposed a command, user can edit
    ReviewAction,

    /// Command is being executed
    Executing,

    /// Sending execution result back to AI
    Finalizing,

    /// Final summary displayed
    Done,
}

impl Default for AppState {
    fn default() -> Self {
        Self::Input
    }
}
