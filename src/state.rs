//! Application state machine
//!
//! Defines the AppState enum and state transition logic.
//! All transition functions are pure (no IO) for testability.

/// Application states following the ReAct pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl AppState {
    /// Returns all possible states (useful for testing)
    pub fn all_states() -> &'static [AppState] {
        &[
            AppState::Input,
            AppState::Thinking,
            AppState::ReviewAction,
            AppState::Executing,
            AppState::Finalizing,
            AppState::Done,
        ]
    }

    /// Check if this state blocks user input
    pub fn blocks_input(&self) -> bool {
        matches!(
            self,
            AppState::Thinking | AppState::Finalizing | AppState::Executing
        )
    }

    /// Check if this state shows a spinner
    pub fn shows_spinner(&self) -> bool {
        matches!(self, AppState::Thinking | AppState::Finalizing)
    }

    /// Get display name for the status bar
    pub fn display_name(&self) -> &'static str {
        match self {
            AppState::Input => "Input",
            AppState::Thinking => "Thinking...",
            AppState::ReviewAction => "Review Command",
            AppState::Executing => "Executing...",
            AppState::Finalizing => "Analyzing...",
            AppState::Done => "Done",
        }
    }
}

/// Result of a state transition attempt
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionResult {
    /// Transition succeeded to new state
    Success(AppState),
    /// Transition was ignored (e.g., empty input)
    Ignored,
    /// Transition failed with error
    Error(String),
}

/// Events that can trigger state transitions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateEvent {
    /// User submitted input (may be empty)
    SubmitInput { is_empty: bool },
    /// User pressed escape
    Escape,
    /// AI responded with a tool call
    ToolCallReceived,
    /// AI responded with plain text
    TextResponseReceived,
    /// API request failed
    ApiError,
    /// User confirmed command execution
    ConfirmCommand,
    /// User cancelled command
    CancelCommand,
    /// Command execution completed
    CommandComplete,
    /// AI analysis completed
    AnalysisComplete,
    /// Continue from Done state
    Continue,
}

/// Pure state transition function
///
/// Given the current state and an event, returns the result of the transition.
/// This function has no side effects and is fully testable.
pub fn transition(current: AppState, event: StateEvent) -> TransitionResult {
    match (current, event) {
        // Input state transitions
        (AppState::Input, StateEvent::SubmitInput { is_empty: true }) => TransitionResult::Ignored,
        (AppState::Input, StateEvent::SubmitInput { is_empty: false }) => {
            TransitionResult::Success(AppState::Thinking)
        }
        (AppState::Input, StateEvent::Escape) => TransitionResult::Success(AppState::Done),

        // Thinking state transitions
        (AppState::Thinking, StateEvent::ToolCallReceived) => {
            TransitionResult::Success(AppState::ReviewAction)
        }
        (AppState::Thinking, StateEvent::TextResponseReceived) => {
            TransitionResult::Success(AppState::Input)
        }
        (AppState::Thinking, StateEvent::ApiError) => TransitionResult::Success(AppState::Input),

        // ReviewAction state transitions
        (AppState::ReviewAction, StateEvent::ConfirmCommand) => {
            TransitionResult::Success(AppState::Executing)
        }
        (AppState::ReviewAction, StateEvent::CancelCommand) => {
            TransitionResult::Success(AppState::Input)
        }
        (AppState::ReviewAction, StateEvent::Escape) => TransitionResult::Success(AppState::Input),

        // Executing state transitions
        (AppState::Executing, StateEvent::CommandComplete) => {
            TransitionResult::Success(AppState::Finalizing)
        }

        // Finalizing state transitions
        (AppState::Finalizing, StateEvent::ToolCallReceived) => {
            TransitionResult::Success(AppState::ReviewAction)
        }
        (AppState::Finalizing, StateEvent::TextResponseReceived) => {
            TransitionResult::Success(AppState::Input)
        }
        (AppState::Finalizing, StateEvent::AnalysisComplete) => {
            TransitionResult::Success(AppState::Input)
        }
        (AppState::Finalizing, StateEvent::ApiError) => TransitionResult::Success(AppState::Input),

        // Done state transitions
        (AppState::Done, StateEvent::Continue) => TransitionResult::Success(AppState::Input),

        // Invalid transitions
        (state, event) => TransitionResult::Error(format!(
            "Invalid transition: {:?} with event {:?}",
            state, event
        )),
    }
}

/// Check if a transition from one state to another is valid
pub fn is_valid_transition(from: AppState, to: AppState) -> bool {
    match (from, to) {
        // From Input
        (AppState::Input, AppState::Thinking) => true,
        (AppState::Input, AppState::Done) => true,
        (AppState::Input, AppState::Input) => true, // Stay in input (empty submit)

        // From Thinking
        (AppState::Thinking, AppState::ReviewAction) => true,
        (AppState::Thinking, AppState::Input) => true,

        // From ReviewAction
        (AppState::ReviewAction, AppState::Executing) => true,
        (AppState::ReviewAction, AppState::Input) => true,

        // From Executing
        (AppState::Executing, AppState::Finalizing) => true,

        // From Finalizing
        (AppState::Finalizing, AppState::ReviewAction) => true,
        (AppState::Finalizing, AppState::Input) => true,

        // From Done
        (AppState::Done, AppState::Input) => true,

        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_default_state_is_input() {
        assert_eq!(AppState::default(), AppState::Input);
    }

    #[test]
    fn test_all_states_returns_all_variants() {
        let states = AppState::all_states();
        assert_eq!(states.len(), 6);
        assert!(states.contains(&AppState::Input));
        assert!(states.contains(&AppState::Thinking));
        assert!(states.contains(&AppState::ReviewAction));
        assert!(states.contains(&AppState::Executing));
        assert!(states.contains(&AppState::Finalizing));
        assert!(states.contains(&AppState::Done));
    }

    #[test]
    fn test_blocks_input() {
        assert!(!AppState::Input.blocks_input());
        assert!(AppState::Thinking.blocks_input());
        assert!(!AppState::ReviewAction.blocks_input());
        assert!(AppState::Executing.blocks_input());
        assert!(AppState::Finalizing.blocks_input());
        assert!(!AppState::Done.blocks_input());
    }

    #[test]
    fn test_input_empty_submit_ignored() {
        let result = transition(AppState::Input, StateEvent::SubmitInput { is_empty: true });
        assert_eq!(result, TransitionResult::Ignored);
    }

    #[test]
    fn test_input_valid_submit_to_thinking() {
        let result = transition(AppState::Input, StateEvent::SubmitInput { is_empty: false });
        assert_eq!(result, TransitionResult::Success(AppState::Thinking));
    }

    #[test]
    fn test_input_escape_to_done() {
        let result = transition(AppState::Input, StateEvent::Escape);
        assert_eq!(result, TransitionResult::Success(AppState::Done));
    }

    #[test]
    fn test_thinking_tool_call_to_review() {
        let result = transition(AppState::Thinking, StateEvent::ToolCallReceived);
        assert_eq!(result, TransitionResult::Success(AppState::ReviewAction));
    }

    #[test]
    fn test_thinking_text_response_to_input() {
        let result = transition(AppState::Thinking, StateEvent::TextResponseReceived);
        assert_eq!(result, TransitionResult::Success(AppState::Input));
    }

    #[test]
    fn test_thinking_api_error_to_input() {
        let result = transition(AppState::Thinking, StateEvent::ApiError);
        assert_eq!(result, TransitionResult::Success(AppState::Input));
    }

    #[test]
    fn test_review_confirm_to_executing() {
        let result = transition(AppState::ReviewAction, StateEvent::ConfirmCommand);
        assert_eq!(result, TransitionResult::Success(AppState::Executing));
    }

    #[test]
    fn test_review_cancel_to_input() {
        let result = transition(AppState::ReviewAction, StateEvent::CancelCommand);
        assert_eq!(result, TransitionResult::Success(AppState::Input));
    }

    #[test]
    fn test_executing_complete_to_finalizing() {
        let result = transition(AppState::Executing, StateEvent::CommandComplete);
        assert_eq!(result, TransitionResult::Success(AppState::Finalizing));
    }

    #[test]
    fn test_finalizing_analysis_complete_to_input() {
        let result = transition(AppState::Finalizing, StateEvent::AnalysisComplete);
        assert_eq!(result, TransitionResult::Success(AppState::Input));
    }

    #[test]
    fn test_invalid_transition_returns_error() {
        let result = transition(
            AppState::Executing,
            StateEvent::SubmitInput { is_empty: false },
        );
        assert!(matches!(result, TransitionResult::Error(_)));
    }

    // Strategy to generate arbitrary AppState
    fn arb_app_state() -> impl Strategy<Value = AppState> {
        prop_oneof![
            Just(AppState::Input),
            Just(AppState::Thinking),
            Just(AppState::ReviewAction),
            Just(AppState::Executing),
            Just(AppState::Finalizing),
            Just(AppState::Done),
        ]
    }

    // Strategy to generate arbitrary StateEvent
    fn arb_state_event() -> impl Strategy<Value = StateEvent> {
        prop_oneof![
            any::<bool>().prop_map(|is_empty| StateEvent::SubmitInput { is_empty }),
            Just(StateEvent::Escape),
            Just(StateEvent::ToolCallReceived),
            Just(StateEvent::TextResponseReceived),
            Just(StateEvent::ApiError),
            Just(StateEvent::ConfirmCommand),
            Just(StateEvent::CancelCommand),
            Just(StateEvent::CommandComplete),
            Just(StateEvent::AnalysisComplete),
            Just(StateEvent::Continue),
        ]
    }

    // **Feature: agent-rs, Property 14: State Validity**
    // *For any* App instance at any point in execution, the state field SHALL be
    // one of the defined AppState variants, and the UI render function SHALL not
    // panic for that state.
    // **Validates: Requirements 7.1, 7.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_state_is_always_valid_variant(state in arb_app_state()) {
            // Property: Every generated state is one of the defined variants
            // This is guaranteed by the type system, but we verify the all_states() list is complete
            let all_states = AppState::all_states();
            prop_assert!(all_states.contains(&state));
        }

        #[test]
        fn prop_transition_result_is_valid_state(
            state in arb_app_state(),
            event in arb_state_event()
        ) {
            // Property: Any transition result that succeeds produces a valid state
            let result = transition(state, event);

            match result {
                TransitionResult::Success(new_state) => {
                    // The new state must be a valid variant
                    let all_states = AppState::all_states();
                    prop_assert!(all_states.contains(&new_state));
                }
                TransitionResult::Ignored => {
                    // Ignored is valid - no state change
                }
                TransitionResult::Error(_) => {
                    // Error is valid - invalid transition was attempted
                }
            }
        }

        #[test]
        fn prop_display_name_never_panics(state in arb_app_state()) {
            // Property: display_name() never panics for any valid state
            let name = state.display_name();
            prop_assert!(!name.is_empty());
        }

        #[test]
        fn prop_blocks_input_is_deterministic(state in arb_app_state()) {
            // Property: blocks_input() returns consistent results
            let result1 = state.blocks_input();
            let result2 = state.blocks_input();
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_shows_spinner_is_deterministic(state in arb_app_state()) {
            // Property: shows_spinner() returns consistent results
            let result1 = state.shows_spinner();
            let result2 = state.shows_spinner();
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_successful_transitions_are_valid(
            state in arb_app_state(),
            event in arb_state_event()
        ) {
            // Property: If a transition succeeds, it must be a valid transition
            let result = transition(state, event);

            if let TransitionResult::Success(new_state) = result {
                prop_assert!(
                    is_valid_transition(state, new_state),
                    "Transition from {:?} to {:?} should be valid",
                    state,
                    new_state
                );
            }
        }
    }
}
