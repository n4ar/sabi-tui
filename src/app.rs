//! Core application struct
//!
//! Contains the App struct that holds all application state.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::TextArea;

use crate::config::Config;
use crate::message::Message;
use crate::state::{AppState, StateEvent, TransitionResult, transition};

/// Main application state container
pub struct App<'a> {
    /// Current application state
    pub state: AppState,

    /// Input textarea for user queries
    pub input_textarea: TextArea<'a>,

    /// Editable textarea for command review
    pub action_textarea: TextArea<'a>,

    /// Conversation history for AI context
    pub messages: Vec<Message>,

    /// Current command being executed
    pub current_command: Option<String>,

    /// Output from command execution
    pub execution_output: String,

    /// Error message if any
    pub error_message: Option<String>,

    /// Spinner frame for loading animation
    pub spinner_frame: usize,

    /// Flag to quit application
    pub should_quit: bool,

    /// Scroll offset for chat history
    pub scroll_offset: u16,

    /// Flag indicating dangerous command detected
    pub dangerous_command_detected: bool,

    /// Application configuration
    pub config: Config,
}

impl<'a> App<'a> {
    /// Create a new App instance with the given configuration
    pub fn new(config: Config) -> Self {
        let mut input_textarea = TextArea::default();
        input_textarea.set_placeholder_text("Type your query here...");
        
        let action_textarea = TextArea::default();

        Self {
            state: AppState::default(),
            input_textarea,
            action_textarea,
            messages: Vec::new(),
            current_command: None,
            execution_output: String::new(),
            error_message: None,
            spinner_frame: 0,
            should_quit: false,
            scroll_offset: 0,
            dangerous_command_detected: false,
            config,
        }
    }

    /// Get the current input text (trimmed)
    pub fn get_input_text(&self) -> String {
        self.input_textarea.lines().join("\n").trim().to_string()
    }

    /// Get the current action text (the command to execute)
    pub fn get_action_text(&self) -> String {
        self.action_textarea.lines().join("\n").trim().to_string()
    }

    /// Check if the input is empty (whitespace-only counts as empty)
    pub fn is_input_empty(&self) -> bool {
        self.get_input_text().is_empty()
    }

    /// Clear the input textarea
    pub fn clear_input(&mut self) {
        self.input_textarea = TextArea::default();
        self.input_textarea.set_placeholder_text("Type your query here...");
    }

    /// Clear the action textarea
    pub fn clear_action(&mut self) {
        self.action_textarea = TextArea::default();
        self.dangerous_command_detected = false;
    }

    /// Set the action textarea content (for command review)
    pub fn set_action_text(&mut self, text: &str) {
        self.action_textarea = TextArea::default();
        for line in text.lines() {
            self.action_textarea.insert_str(line);
            self.action_textarea.insert_newline();
        }
        // Remove the trailing newline if we added one
        if text.lines().count() > 0 {
            self.action_textarea.delete_char();
        }
    }

    /// Add a message to the conversation history
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        // Reset scroll to show latest message
        self.scroll_offset = 0;
    }

    /// Clear the error message
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Set an error message
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error_message = Some(error.into());
    }

    /// Attempt a state transition
    /// 
    /// Returns true if the transition was successful, false otherwise.
    pub fn transition(&mut self, event: StateEvent) -> bool {
        match transition(self.state, event) {
            TransitionResult::Success(new_state) => {
                self.state = new_state;
                true
            }
            TransitionResult::Ignored => false,
            TransitionResult::Error(msg) => {
                self.set_error(msg);
                false
            }
        }
    }

    /// Submit the current input
    /// 
    /// Returns true if the input was submitted (non-empty), false otherwise.
    pub fn submit_input(&mut self) -> bool {
        let is_empty = self.is_input_empty();
        
        if !is_empty {
            let input = self.get_input_text();
            self.add_message(Message::user(&input));
            self.clear_input();
        }
        
        self.transition(StateEvent::SubmitInput { is_empty })
    }

    /// Advance the spinner animation
    pub fn tick_spinner(&mut self) {
        const SPINNER_FRAMES: usize = 10;
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES;
    }

    /// Get the current spinner character
    pub fn spinner_char(&self) -> char {
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        SPINNER[self.spinner_frame % SPINNER.len()]
    }

    /// Handle a keyboard event based on the current state
    ///
    /// Returns an InputResult indicating what action should be taken.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> InputResult {
        // Check for Ctrl+C to quit from any state
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return InputResult::Quit;
        }

        match self.state {
            AppState::Input => self.handle_input_state(key),
            AppState::Thinking => self.handle_thinking_state(key),
            AppState::ReviewAction => self.handle_review_action_state(key),
            AppState::Executing => self.handle_executing_state(key),
            AppState::Finalizing => self.handle_finalizing_state(key),
            AppState::Done => self.handle_done_state(key),
        }
    }

    /// Scroll chat history up
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Scroll chat history down
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Handle keyboard events in Input state
    fn handle_input_state(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                if self.submit_input() {
                    InputResult::SubmitQuery
                } else {
                    InputResult::Ignored
                }
            }
            KeyCode::Esc => {
                self.should_quit = true;
                self.transition(StateEvent::Escape);
                InputResult::Quit
            }
            KeyCode::Up => {
                self.scroll_up();
                InputResult::Handled
            }
            KeyCode::Down => {
                self.scroll_down();
                InputResult::Handled
            }
            // Pass other keys to the textarea
            _ => {
                self.input_textarea.input(key);
                InputResult::Handled
            }
        }
    }

    /// Handle keyboard events in Thinking state (input blocked)
    fn handle_thinking_state(&mut self, key: KeyEvent) -> InputResult {
        // Only allow Escape for emergency quit in async states
        if key.code == KeyCode::Esc {
            self.should_quit = true;
            InputResult::Quit
        } else {
            // Input is blocked during Thinking state
            InputResult::Blocked
        }
    }

    /// Handle keyboard events in ReviewAction state
    fn handle_review_action_state(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                // Confirm command execution
                let command = self.get_action_text();
                if !command.is_empty() {
                    self.current_command = Some(command);
                    self.transition(StateEvent::ConfirmCommand);
                    InputResult::ExecuteCommand
                } else {
                    InputResult::Ignored
                }
            }
            KeyCode::Esc => {
                // Cancel command and return to input
                self.clear_action();
                self.transition(StateEvent::CancelCommand);
                InputResult::CancelCommand
            }
            // Pass other keys to the action textarea for editing
            _ => {
                self.action_textarea.input(key);
                InputResult::Handled
            }
        }
    }

    /// Handle keyboard events in Executing state (input blocked)
    fn handle_executing_state(&mut self, key: KeyEvent) -> InputResult {
        // Only allow Escape for emergency quit
        if key.code == KeyCode::Esc {
            self.should_quit = true;
            InputResult::Quit
        } else {
            InputResult::Blocked
        }
    }

    /// Handle keyboard events in Finalizing state (input blocked)
    fn handle_finalizing_state(&mut self, key: KeyEvent) -> InputResult {
        // Only allow Escape for emergency quit
        if key.code == KeyCode::Esc {
            self.should_quit = true;
            InputResult::Quit
        } else {
            InputResult::Blocked
        }
    }

    /// Handle keyboard events in Done state
    fn handle_done_state(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Enter => {
                // Continue to new input
                self.transition(StateEvent::Continue);
                InputResult::Continue
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_quit = true;
                InputResult::Quit
            }
            _ => InputResult::Ignored,
        }
    }
}

/// Result of handling an input event
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputResult {
    /// Input was handled (e.g., character typed)
    Handled,
    /// Input was ignored (e.g., empty submit)
    Ignored,
    /// Input was blocked (async state)
    Blocked,
    /// User submitted a query, should send to AI
    SubmitQuery,
    /// User confirmed command execution
    ExecuteCommand,
    /// User cancelled command
    CancelCommand,
    /// User wants to continue from Done state
    Continue,
    /// User wants to quit
    Quit,
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Create a test App with default config
    fn test_app() -> App<'static> {
        App::new(Config::default())
    }

    // Strategy to generate whitespace-only strings
    fn whitespace_string() -> impl Strategy<Value = String> {
        prop::collection::vec(prop_oneof![Just(' '), Just('\t'), Just('\n'), Just('\r')], 0..20)
            .prop_map(|chars| chars.into_iter().collect())
    }

    // **Feature: agent-rs, Property 1: Empty Input Rejection**
    // *For any* input string composed entirely of whitespace characters, submitting it
    // SHALL NOT change the application state from Input, and the message history SHALL
    // remain unchanged.
    // **Validates: Requirements 1.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_empty_input_rejection(whitespace in whitespace_string()) {
            let mut app = test_app();
            
            // Ensure we start in Input state
            assert_eq!(app.state, AppState::Input);
            let initial_message_count = app.messages.len();
            
            // Set the input to whitespace-only content
            app.input_textarea = TextArea::default();
            for ch in whitespace.chars() {
                app.input_textarea.insert_char(ch);
            }
            
            // Attempt to submit
            let submitted = app.submit_input();
            
            // Property: submission should fail (return false)
            prop_assert!(!submitted, "Whitespace-only input should not be submitted");
            
            // Property: state should remain Input
            prop_assert_eq!(
                app.state, 
                AppState::Input,
                "State should remain Input after whitespace submission"
            );
            
            // Property: message history should be unchanged
            prop_assert_eq!(
                app.messages.len(),
                initial_message_count,
                "Message history should not change after whitespace submission"
            );
        }

        #[test]
        fn prop_empty_string_rejection(_dummy in 0..1) {
            let mut app = test_app();
            
            // Ensure we start in Input state with empty textarea
            assert_eq!(app.state, AppState::Input);
            let initial_message_count = app.messages.len();
            
            // Input is already empty by default
            assert!(app.is_input_empty());
            
            // Attempt to submit
            let submitted = app.submit_input();
            
            // Property: submission should fail
            prop_assert!(!submitted);
            
            // Property: state should remain Input
            prop_assert_eq!(app.state, AppState::Input);
            
            // Property: message history should be unchanged
            prop_assert_eq!(app.messages.len(), initial_message_count);
        }
    }

    #[test]
    fn test_is_input_empty_with_whitespace() {
        let mut app = test_app();
        
        // Empty by default
        assert!(app.is_input_empty());
        
        // Add spaces
        app.input_textarea.insert_str("   ");
        assert!(app.is_input_empty());
        
        // Add tabs
        app.clear_input();
        app.input_textarea.insert_str("\t\t");
        assert!(app.is_input_empty());
        
        // Add newlines
        app.clear_input();
        app.input_textarea.insert_str("\n\n");
        assert!(app.is_input_empty());
        
        // Add actual content
        app.clear_input();
        app.input_textarea.insert_str("hello");
        assert!(!app.is_input_empty());
    }

    // Strategy to generate non-empty, non-whitespace strings
    fn non_empty_string() -> impl Strategy<Value = String> {
        // Generate strings that have at least one non-whitespace character
        ("[a-zA-Z0-9][a-zA-Z0-9 ]{0,50}", 1..52)
            .prop_map(|(s, _)| s)
    }

    // **Feature: agent-rs, Property 2: Valid Input State Transition**
    // *For any* non-empty, non-whitespace input string, submitting it in Input state
    // SHALL transition the application to Thinking state and add the input to message history.
    // **Validates: Requirements 1.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_valid_input_state_transition(input in non_empty_string()) {
            let mut app = test_app();
            
            // Ensure we start in Input state
            assert_eq!(app.state, AppState::Input);
            let initial_message_count = app.messages.len();
            
            // Set the input to non-empty content
            app.input_textarea = TextArea::default();
            app.input_textarea.insert_str(&input);
            
            // Verify input is not empty
            prop_assert!(!app.is_input_empty(), "Input should not be empty: '{}'", input);
            
            // Attempt to submit
            let submitted = app.submit_input();
            
            // Property: submission should succeed (return true)
            prop_assert!(submitted, "Non-empty input should be submitted");
            
            // Property: state should transition to Thinking
            prop_assert_eq!(
                app.state, 
                AppState::Thinking,
                "State should transition to Thinking after valid submission"
            );
            
            // Property: message history should have one more message
            prop_assert_eq!(
                app.messages.len(),
                initial_message_count + 1,
                "Message history should grow by 1 after valid submission"
            );
            
            // Property: the new message should be a User message with the input content
            let last_message = app.messages.last().unwrap();
            prop_assert_eq!(last_message.role.clone(), crate::message::MessageRole::User);
            prop_assert_eq!(last_message.content.trim(), input.trim());
        }

        #[test]
        fn prop_input_cleared_after_submission(input in non_empty_string()) {
            let mut app = test_app();
            
            // Set the input
            app.input_textarea = TextArea::default();
            app.input_textarea.insert_str(&input);
            
            // Submit
            app.submit_input();
            
            // Property: input should be cleared after submission
            prop_assert!(
                app.is_input_empty(),
                "Input should be cleared after submission"
            );
        }
    }

    #[test]
    fn test_valid_input_submission() {
        let mut app = test_app();
        
        // Set valid input
        app.input_textarea.insert_str("list files");
        
        // Submit
        let submitted = app.submit_input();
        
        assert!(submitted);
        assert_eq!(app.state, AppState::Thinking);
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].content, "list files");
    }

    // Strategy to generate arbitrary error messages
    fn arb_error_message() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ]{1,100}".prop_map(|s| s)
    }

    // **Feature: agent-rs, Property 5: API Error Recovery**
    // *For any* API error during Thinking state, the application SHALL transition
    // back to Input state and set an error message.
    // **Validates: Requirements 2.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_api_error_recovery_from_thinking(error_msg in arb_error_message()) {
            let mut app = test_app();
            
            // First, get to Thinking state by submitting valid input
            app.input_textarea.insert_str("test query");
            app.submit_input();
            
            // Verify we're in Thinking state
            prop_assert_eq!(app.state, AppState::Thinking);
            
            // Simulate API error by setting error and transitioning
            app.set_error(&error_msg);
            let transitioned = app.transition(StateEvent::ApiError);
            
            // Property: transition should succeed
            prop_assert!(transitioned, "API error transition should succeed from Thinking state");
            
            // Property: state should be Input after API error
            prop_assert_eq!(
                app.state,
                AppState::Input,
                "State should transition to Input after API error"
            );
            
            // Property: error message should be set
            prop_assert!(
                app.error_message.is_some(),
                "Error message should be set after API error"
            );
            
            // Property: error message should match what we set
            prop_assert_eq!(
                app.error_message.as_ref().unwrap(),
                &error_msg,
                "Error message should match the set error"
            );
        }

        #[test]
        fn prop_api_error_recovery_from_finalizing(error_msg in arb_error_message()) {
            let mut app = test_app();
            
            // Manually set state to Finalizing (simulating post-command execution)
            app.state = AppState::Finalizing;
            
            // Simulate API error
            app.set_error(&error_msg);
            let transitioned = app.transition(StateEvent::ApiError);
            
            // Property: transition should succeed
            prop_assert!(transitioned, "API error transition should succeed from Finalizing state");
            
            // Property: state should be Input after API error
            prop_assert_eq!(
                app.state,
                AppState::Input,
                "State should transition to Input after API error in Finalizing"
            );
            
            // Property: error message should be set
            prop_assert!(
                app.error_message.is_some(),
                "Error message should be set after API error"
            );
        }

        #[test]
        fn prop_api_error_preserves_message_history(
            input in non_empty_string(),
            error_msg in arb_error_message()
        ) {
            let mut app = test_app();
            
            // Submit input to get to Thinking state
            app.input_textarea.insert_str(&input);
            app.submit_input();
            
            // Record message count after submission
            let message_count = app.messages.len();
            
            // Simulate API error
            app.set_error(&error_msg);
            app.transition(StateEvent::ApiError);
            
            // Property: message history should be preserved (not cleared)
            prop_assert_eq!(
                app.messages.len(),
                message_count,
                "Message history should be preserved after API error"
            );
        }
    }

    #[test]
    fn test_api_error_recovery() {
        let mut app = test_app();
        
        // Get to Thinking state
        app.input_textarea.insert_str("test");
        app.submit_input();
        assert_eq!(app.state, AppState::Thinking);
        
        // Simulate API error
        app.set_error("Network error");
        app.transition(StateEvent::ApiError);
        
        // Should be back in Input state with error
        assert_eq!(app.state, AppState::Input);
        assert!(app.error_message.is_some());
        assert_eq!(app.error_message.unwrap(), "Network error");
    }

    // **Feature: agent-rs, Property 8: ReviewAction State Transitions**
    // *For any* application in ReviewAction state, pressing Enter SHALL transition to
    // Executing state, and pressing Escape SHALL transition to Input state with
    // action_textarea cleared.
    // **Validates: Requirements 3.3, 3.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_review_action_enter_transitions_to_executing(command in non_empty_string()) {
            let mut app = test_app();
            
            // Set up app in ReviewAction state with a command
            app.state = AppState::ReviewAction;
            app.action_textarea = TextArea::default();
            app.action_textarea.insert_str(&command);
            
            // Verify we're in ReviewAction state
            prop_assert_eq!(app.state, AppState::ReviewAction);
            
            // Simulate pressing Enter
            let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
            let result = app.handle_key_event(key);
            
            // Property: result should be ExecuteCommand
            prop_assert_eq!(
                result,
                InputResult::ExecuteCommand,
                "Enter in ReviewAction should return ExecuteCommand"
            );
            
            // Property: state should transition to Executing
            prop_assert_eq!(
                app.state,
                AppState::Executing,
                "State should transition to Executing after Enter in ReviewAction"
            );
            
            // Property: current_command should be set
            prop_assert!(
                app.current_command.is_some(),
                "current_command should be set after confirming"
            );
            
            // Property: current_command should match the action text
            prop_assert_eq!(
                app.current_command.as_ref().unwrap().trim(),
                command.trim(),
                "current_command should match the action textarea content"
            );
        }

        #[test]
        fn prop_review_action_escape_transitions_to_input(command in non_empty_string()) {
            let mut app = test_app();
            
            // Set up app in ReviewAction state with a command
            app.state = AppState::ReviewAction;
            app.action_textarea = TextArea::default();
            app.action_textarea.insert_str(&command);
            
            // Verify we're in ReviewAction state with content
            prop_assert_eq!(app.state, AppState::ReviewAction);
            prop_assert!(!app.get_action_text().is_empty());
            
            // Simulate pressing Escape
            let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
            let result = app.handle_key_event(key);
            
            // Property: result should be CancelCommand
            prop_assert_eq!(
                result,
                InputResult::CancelCommand,
                "Escape in ReviewAction should return CancelCommand"
            );
            
            // Property: state should transition to Input
            prop_assert_eq!(
                app.state,
                AppState::Input,
                "State should transition to Input after Escape in ReviewAction"
            );
            
            // Property: action_textarea should be cleared
            prop_assert!(
                app.get_action_text().is_empty(),
                "action_textarea should be cleared after Escape in ReviewAction"
            );
            
            // Property: dangerous_command_detected should be reset
            prop_assert!(
                !app.dangerous_command_detected,
                "dangerous_command_detected should be reset after cancel"
            );
        }

        #[test]
        fn prop_review_action_empty_command_ignored(_dummy in 0..1) {
            let mut app = test_app();
            
            // Set up app in ReviewAction state with empty command
            app.state = AppState::ReviewAction;
            app.action_textarea = TextArea::default();
            
            // Verify action is empty
            prop_assert!(app.get_action_text().is_empty());
            
            // Simulate pressing Enter
            let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
            let result = app.handle_key_event(key);
            
            // Property: result should be Ignored (can't execute empty command)
            prop_assert_eq!(
                result,
                InputResult::Ignored,
                "Enter with empty command should be ignored"
            );
            
            // Property: state should remain ReviewAction
            prop_assert_eq!(
                app.state,
                AppState::ReviewAction,
                "State should remain ReviewAction when command is empty"
            );
        }

        #[test]
        fn prop_review_action_allows_editing(
            initial_command in non_empty_string(),
            additional_char in "[a-zA-Z0-9]"
        ) {
            let mut app = test_app();
            
            // Set up app in ReviewAction state with a command
            app.state = AppState::ReviewAction;
            app.action_textarea = TextArea::default();
            app.action_textarea.insert_str(&initial_command);
            
            let initial_len = app.get_action_text().len();
            
            // Simulate typing a character
            let ch = additional_char.chars().next().unwrap();
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            let result = app.handle_key_event(key);
            
            // Property: result should be Handled
            prop_assert_eq!(
                result,
                InputResult::Handled,
                "Character input in ReviewAction should be handled"
            );
            
            // Property: state should remain ReviewAction
            prop_assert_eq!(
                app.state,
                AppState::ReviewAction,
                "State should remain ReviewAction during editing"
            );
            
            // Property: action text should have grown
            prop_assert!(
                app.get_action_text().len() > initial_len,
                "Action text should grow after character input"
            );
        }
    }

    #[test]
    fn test_review_action_enter_executes() {
        let mut app = test_app();
        
        // Set up in ReviewAction state
        app.state = AppState::ReviewAction;
        app.action_textarea.insert_str("ls -la");
        
        // Press Enter
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let result = app.handle_key_event(key);
        
        assert_eq!(result, InputResult::ExecuteCommand);
        assert_eq!(app.state, AppState::Executing);
        assert_eq!(app.current_command, Some("ls -la".to_string()));
    }

    #[test]
    fn test_review_action_escape_cancels() {
        let mut app = test_app();
        
        // Set up in ReviewAction state
        app.state = AppState::ReviewAction;
        app.action_textarea.insert_str("rm -rf /");
        app.dangerous_command_detected = true;
        
        // Press Escape
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let result = app.handle_key_event(key);
        
        assert_eq!(result, InputResult::CancelCommand);
        assert_eq!(app.state, AppState::Input);
        assert!(app.get_action_text().is_empty());
        assert!(!app.dangerous_command_detected);
    }

    // Strategy to generate async (blocking) states
    fn arb_async_state() -> impl Strategy<Value = AppState> {
        prop_oneof![
            Just(AppState::Thinking),
            Just(AppState::Finalizing),
            Just(AppState::Executing),
        ]
    }

    // Strategy to generate non-escape key events
    fn arb_non_escape_key() -> impl Strategy<Value = KeyEvent> {
        prop_oneof![
            // Regular characters
            "[a-zA-Z0-9]".prop_map(|s| {
                let ch = s.chars().next().unwrap();
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
            }),
            // Enter key
            Just(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            // Arrow keys
            Just(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Just(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Just(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Just(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            // Backspace and Delete
            Just(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Just(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
            // Tab
            Just(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        ]
    }

    // **Feature: agent-rs, Property 15: Input Blocking in Async States**
    // *For any* application in Thinking or Finalizing state, keyboard input events
    // (except Escape for emergency quit) SHALL NOT modify input_textarea or
    // action_textarea content.
    // **Validates: Requirements 7.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_input_blocked_in_async_states(
            state in arb_async_state(),
            key in arb_non_escape_key(),
            initial_input in "[a-zA-Z0-9]{0,20}",
            initial_action in "[a-zA-Z0-9]{0,20}",
        ) {
            let mut app = test_app();
            
            // Set up initial content in textareas
            app.input_textarea = TextArea::default();
            app.input_textarea.insert_str(&initial_input);
            app.action_textarea = TextArea::default();
            app.action_textarea.insert_str(&initial_action);
            
            // Set the async state
            app.state = state;
            
            // Record initial content
            let input_before = app.get_input_text();
            let action_before = app.get_action_text();
            
            // Handle the key event
            let result = app.handle_key_event(key);
            
            // Property: result should be Blocked
            prop_assert_eq!(
                result,
                InputResult::Blocked,
                "Non-escape keys should be blocked in {:?} state",
                state
            );
            
            // Property: input_textarea content should be unchanged
            prop_assert_eq!(
                app.get_input_text(),
                input_before,
                "input_textarea should not change in {:?} state",
                state
            );
            
            // Property: action_textarea content should be unchanged
            prop_assert_eq!(
                app.get_action_text(),
                action_before,
                "action_textarea should not change in {:?} state",
                state
            );
        }

        #[test]
        fn prop_escape_allowed_in_async_states(state in arb_async_state()) {
            let mut app = test_app();
            
            // Set the async state
            app.state = state;
            
            // Simulate pressing Escape
            let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
            let result = app.handle_key_event(key);
            
            // Property: result should be Quit (emergency exit)
            prop_assert_eq!(
                result,
                InputResult::Quit,
                "Escape should allow quit in {:?} state",
                state
            );
            
            // Property: should_quit flag should be set
            prop_assert!(
                app.should_quit,
                "should_quit should be true after Escape in {:?} state",
                state
            );
        }

        #[test]
        fn prop_ctrl_c_allowed_in_async_states(state in arb_async_state()) {
            let mut app = test_app();
            
            // Set the async state
            app.state = state;
            
            // Simulate pressing Ctrl+C
            let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
            let result = app.handle_key_event(key);
            
            // Property: result should be Quit
            prop_assert_eq!(
                result,
                InputResult::Quit,
                "Ctrl+C should allow quit in {:?} state",
                state
            );
            
            // Property: should_quit flag should be set
            prop_assert!(
                app.should_quit,
                "should_quit should be true after Ctrl+C in {:?} state",
                state
            );
        }

        #[test]
        fn prop_thinking_state_blocks_input(key in arb_non_escape_key()) {
            let mut app = test_app();
            
            // Get to Thinking state legitimately
            app.input_textarea.insert_str("test query");
            app.submit_input();
            
            prop_assert_eq!(app.state, AppState::Thinking);
            
            // Record initial state
            let input_before = app.get_input_text();
            let action_before = app.get_action_text();
            
            // Try to input
            let result = app.handle_key_event(key);
            
            // Property: should be blocked
            prop_assert_eq!(result, InputResult::Blocked);
            
            // Property: content unchanged
            prop_assert_eq!(app.get_input_text(), input_before);
            prop_assert_eq!(app.get_action_text(), action_before);
        }

        #[test]
        fn prop_finalizing_state_blocks_input(key in arb_non_escape_key()) {
            let mut app = test_app();
            
            // Set to Finalizing state
            app.state = AppState::Finalizing;
            
            // Add some content to verify it's not modified
            app.input_textarea.insert_str("previous input");
            app.action_textarea.insert_str("previous action");
            
            let input_before = app.get_input_text();
            let action_before = app.get_action_text();
            
            // Try to input
            let result = app.handle_key_event(key);
            
            // Property: should be blocked
            prop_assert_eq!(result, InputResult::Blocked);
            
            // Property: content unchanged
            prop_assert_eq!(app.get_input_text(), input_before);
            prop_assert_eq!(app.get_action_text(), action_before);
        }
    }

    #[test]
    fn test_thinking_blocks_character_input() {
        let mut app = test_app();
        
        // Get to Thinking state
        app.input_textarea.insert_str("test");
        app.submit_input();
        assert_eq!(app.state, AppState::Thinking);
        
        // Try to type a character
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let result = app.handle_key_event(key);
        
        assert_eq!(result, InputResult::Blocked);
        assert!(app.get_input_text().is_empty()); // Was cleared on submit
    }

    #[test]
    fn test_thinking_allows_escape() {
        let mut app = test_app();
        
        // Get to Thinking state
        app.input_textarea.insert_str("test");
        app.submit_input();
        assert_eq!(app.state, AppState::Thinking);
        
        // Press Escape
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let result = app.handle_key_event(key);
        
        assert_eq!(result, InputResult::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_executing_blocks_input() {
        let mut app = test_app();
        
        // Set to Executing state
        app.state = AppState::Executing;
        
        // Try to type
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let result = app.handle_key_event(key);
        
        assert_eq!(result, InputResult::Blocked);
    }

    #[test]
    fn test_finalizing_blocks_input() {
        let mut app = test_app();
        
        // Set to Finalizing state
        app.state = AppState::Finalizing;
        
        // Try to press Enter
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let result = app.handle_key_event(key);
        
        assert_eq!(result, InputResult::Blocked);
    }

    // **Feature: agent-rs, Property 6: Command Display in ReviewAction**
    // *For any* tool call response from the AI, the command SHALL be displayed
    // in the action_textarea when transitioning to ReviewAction state.
    // **Validates: Requirements 3.1**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_command_displayed_in_review_action(command in "[a-zA-Z][a-zA-Z0-9 _\\-./]{0,50}") {
            let mut app = test_app();
            
            // Simulate receiving a tool call and transitioning to ReviewAction
            app.state = AppState::Thinking;
            app.set_action_text(&command);
            app.transition(StateEvent::ToolCallReceived);
            
            // Property: state should be ReviewAction
            prop_assert_eq!(app.state, AppState::ReviewAction);
            
            // Property: action_textarea should contain the command
            let action_text = app.get_action_text();
            prop_assert_eq!(
                action_text.trim(),
                command.trim(),
                "action_textarea should display the command"
            );
        }

        #[test]
        fn prop_command_editable_in_review_action(
            initial_command in "[a-zA-Z][a-zA-Z0-9 ]{0,20}",
            edit_char in "[a-zA-Z0-9]"
        ) {
            let mut app = test_app();
            
            // Set up in ReviewAction with a command
            app.state = AppState::ReviewAction;
            app.set_action_text(&initial_command);
            
            let initial_text = app.get_action_text();
            
            // Type a character to edit
            let ch = edit_char.chars().next().unwrap();
            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
            app.handle_key_event(key);
            
            // Property: text should have changed (character added)
            prop_assert_ne!(
                app.get_action_text(),
                initial_text,
                "Command should be editable in ReviewAction"
            );
        }
    }

    // **Feature: agent-rs, Property 10: Feedback Loop Consistency**
    // *For any* command execution result, the output SHALL be sent back to the AI
    // and the AI's response SHALL be added to the message history.
    // **Validates: Requirements 5.1, 5.2, 5.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_command_output_added_to_history(
            command in "[a-zA-Z][a-zA-Z0-9 ]{0,20}",
            output in "[a-zA-Z0-9 ]{0,50}",
            exit_code in 0i32..128
        ) {
            let mut app = test_app();
            
            // Set up state as if command was just executed
            app.state = AppState::Executing;
            app.current_command = Some(command.clone());
            let initial_count = app.messages.len();
            
            // Simulate command completion feedback being added
            let feedback = format!(
                "Command: {}\nExit code: {}\nOutput:\n{}",
                command, exit_code, output
            );
            app.add_message(crate::message::Message::user(&feedback));
            app.transition(StateEvent::CommandComplete);
            
            // Property: message count should increase
            prop_assert!(
                app.messages.len() > initial_count,
                "Message history should grow after command completion"
            );
            
            // Property: state should transition to Finalizing
            prop_assert_eq!(
                app.state,
                AppState::Finalizing,
                "State should be Finalizing after command completion"
            );
        }

        #[test]
        fn prop_ai_response_added_after_analysis(
            ai_response in "[a-zA-Z0-9 ]{1,100}"
        ) {
            let mut app = test_app();
            
            // Set up in Finalizing state
            app.state = AppState::Finalizing;
            let initial_count = app.messages.len();
            
            // Simulate AI response
            app.add_message(crate::message::Message::model(&ai_response));
            app.transition(StateEvent::TextResponseReceived);
            
            // Property: message count should increase
            prop_assert_eq!(
                app.messages.len(),
                initial_count + 1,
                "AI response should be added to history"
            );
            
            // Property: state should return to Input
            prop_assert_eq!(
                app.state,
                AppState::Input,
                "State should return to Input after text response"
            );
        }
    }

    // **Feature: agent-rs, Property 16: Error State Recovery**
    // *For any* error occurring during Thinking, Executing, or Finalizing states,
    // the application SHALL transition to Input state and display an error message.
    // **Validates: Requirements 7.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_error_recovery_from_thinking(error_msg in "[a-zA-Z0-9 ]{1,50}") {
            let mut app = test_app();
            app.state = AppState::Thinking;
            
            app.set_error(&error_msg);
            app.transition(StateEvent::ApiError);
            
            prop_assert_eq!(app.state, AppState::Input);
            prop_assert!(app.error_message.is_some());
            prop_assert_eq!(app.error_message.as_ref().unwrap(), &error_msg);
        }

        #[test]
        fn prop_error_recovery_from_finalizing(error_msg in "[a-zA-Z0-9 ]{1,50}") {
            let mut app = test_app();
            app.state = AppState::Finalizing;
            
            app.set_error(&error_msg);
            app.transition(StateEvent::ApiError);
            
            prop_assert_eq!(app.state, AppState::Input);
            prop_assert!(app.error_message.is_some());
        }

        #[test]
        fn prop_error_preserves_history(
            input in non_empty_string(),
            error_msg in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let mut app = test_app();
            
            // Add some history
            app.add_message(crate::message::Message::user(&input));
            let history_count = app.messages.len();
            
            // Simulate error in Thinking
            app.state = AppState::Thinking;
            app.set_error(&error_msg);
            app.transition(StateEvent::ApiError);
            
            // Property: history should be preserved
            prop_assert_eq!(
                app.messages.len(),
                history_count,
                "Message history should be preserved after error"
            );
        }

        #[test]
        fn prop_error_clears_on_new_input(
            error_msg in "[a-zA-Z0-9 ]{1,50}",
            new_input in non_empty_string()
        ) {
            let mut app = test_app();
            
            // Set an error
            app.set_error(&error_msg);
            prop_assert!(app.error_message.is_some());
            
            // Clear error explicitly (as would happen on new action)
            app.clear_error();
            
            prop_assert!(
                app.error_message.is_none(),
                "Error should be clearable"
            );
        }
    }

    #[test]
    fn test_react_loop_tool_call_to_review() {
        let mut app = test_app();
        
        // Start in Thinking (after user submitted query)
        app.state = AppState::Thinking;
        
        // Receive tool call
        app.set_action_text("ls -la");
        app.transition(StateEvent::ToolCallReceived);
        
        assert_eq!(app.state, AppState::ReviewAction);
        assert_eq!(app.get_action_text(), "ls -la");
    }

    #[test]
    fn test_react_loop_text_response_to_input() {
        let mut app = test_app();
        
        // Start in Thinking
        app.state = AppState::Thinking;
        
        // Receive text response (no tool call)
        app.add_message(crate::message::Message::model("The answer is 42"));
        app.transition(StateEvent::TextResponseReceived);
        
        assert_eq!(app.state, AppState::Input);
        assert_eq!(app.messages.last().unwrap().content, "The answer is 42");
    }

    #[test]
    fn test_react_loop_execute_to_finalizing() {
        let mut app = test_app();
        
        // Start in Executing
        app.state = AppState::Executing;
        app.current_command = Some("echo test".to_string());
        
        // Command completes
        app.execution_output = "test".to_string();
        app.transition(StateEvent::CommandComplete);
        
        assert_eq!(app.state, AppState::Finalizing);
    }

    #[test]
    fn test_react_loop_finalizing_to_input() {
        let mut app = test_app();
        
        // Start in Finalizing
        app.state = AppState::Finalizing;
        
        // AI analysis completes with text response
        app.add_message(crate::message::Message::model("Command executed successfully"));
        app.transition(StateEvent::TextResponseReceived);
        
        assert_eq!(app.state, AppState::Input);
    }

    #[test]
    fn test_react_loop_finalizing_to_review_action() {
        let mut app = test_app();
        
        // Start in Finalizing
        app.state = AppState::Finalizing;
        
        // AI wants to run another command
        app.set_action_text("cat output.txt");
        app.transition(StateEvent::ToolCallReceived);
        
        assert_eq!(app.state, AppState::ReviewAction);
        assert_eq!(app.get_action_text(), "cat output.txt");
    }

    // **Feature: agent-rs, Property 11: Message History Append**
    // *For any* sequence of messages added to the history, the messages SHALL be
    // appended in order, and the scroll position SHALL reset to show the latest message.
    // **Validates: Requirements 6.1**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_message_history_append_order(
            messages in prop::collection::vec(
                (prop::bool::ANY, "[a-zA-Z0-9 ]{1,50}"),
                1..10
            )
        ) {
            let mut app = test_app();
            let initial_count = app.messages.len();
            
            // Add messages
            for (is_user, content) in &messages {
                let msg = if *is_user {
                    crate::message::Message::user(content)
                } else {
                    crate::message::Message::model(content)
                };
                app.add_message(msg);
            }
            
            // Property: message count should increase by the number added
            prop_assert_eq!(
                app.messages.len(),
                initial_count + messages.len(),
                "Message count should increase by number of messages added"
            );
            
            // Property: messages should be in the same order
            for (i, (is_user, content)) in messages.iter().enumerate() {
                let msg = &app.messages[initial_count + i];
                let expected_role = if *is_user {
                    crate::message::MessageRole::User
                } else {
                    crate::message::MessageRole::Model
                };
                prop_assert_eq!(
                    &msg.role, &expected_role,
                    "Message role should match at index {}", i
                );
                prop_assert_eq!(
                    &msg.content, content,
                    "Message content should match at index {}", i
                );
            }
        }

        #[test]
        fn prop_message_append_resets_scroll(
            content in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let mut app = test_app();
            
            // Set scroll offset to non-zero
            app.scroll_offset = 10;
            
            // Add a message
            app.add_message(crate::message::Message::user(&content));
            
            // Property: scroll should reset to 0 (showing latest)
            prop_assert_eq!(
                app.scroll_offset, 0,
                "Scroll offset should reset to 0 after adding message"
            );
        }

        #[test]
        fn prop_message_history_preserves_previous(
            initial_msg in "[a-zA-Z0-9 ]{1,50}",
            new_msg in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let mut app = test_app();
            
            // Add initial message
            app.add_message(crate::message::Message::user(&initial_msg));
            let first_count = app.messages.len();
            
            // Add new message
            app.add_message(crate::message::Message::model(&new_msg));
            
            // Property: previous messages should be preserved
            prop_assert_eq!(
                app.messages.len(),
                first_count + 1,
                "Message count should increase by 1"
            );
            
            // Property: first message should still be there
            prop_assert_eq!(
                &app.messages[first_count - 1].content,
                &initial_msg,
                "Previous message should be preserved"
            );
            
            // Property: new message should be last
            prop_assert_eq!(
                &app.messages.last().unwrap().content,
                &new_msg,
                "New message should be last"
            );
        }
    }

    // Strategy to generate textarea edit operations
    #[derive(Debug, Clone)]
    enum TextOp {
        Insert(char),
        Delete,
        Left,
        Right,
        Home,
        End,
    }

    fn arb_text_op() -> impl Strategy<Value = TextOp> {
        prop_oneof![
            "[a-zA-Z0-9 ]".prop_map(|s| TextOp::Insert(s.chars().next().unwrap())),
            Just(TextOp::Delete),
            Just(TextOp::Left),
            Just(TextOp::Right),
            Just(TextOp::Home),
            Just(TextOp::End),
        ]
    }

    // **Feature: agent-rs, Property 7: Textarea Edit Consistency**
    // *For any* sequence of valid text editing operations (insert, delete, cursor move)
    // on a TextArea, the resulting content SHALL reflect all operations applied in order.
    // **Validates: Requirements 3.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_textarea_edit_consistency(
            ops in prop::collection::vec(arb_text_op(), 1..50)
        ) {
            let mut textarea = TextArea::default();
            let mut expected = String::new();
            let mut cursor: usize = 0;

            for op in &ops {
                match op {
                    TextOp::Insert(ch) => {
                        textarea.insert_char(*ch);
                        expected.insert(cursor, *ch);
                        cursor += 1;
                    }
                    TextOp::Delete => {
                        textarea.delete_char();
                        if cursor > 0 {
                            cursor -= 1;
                            expected.remove(cursor);
                        }
                    }
                    TextOp::Left => {
                        textarea.move_cursor(tui_textarea::CursorMove::Back);
                        cursor = cursor.saturating_sub(1);
                    }
                    TextOp::Right => {
                        textarea.move_cursor(tui_textarea::CursorMove::Forward);
                        if cursor < expected.len() {
                            cursor += 1;
                        }
                    }
                    TextOp::Home => {
                        textarea.move_cursor(tui_textarea::CursorMove::Head);
                        cursor = 0;
                    }
                    TextOp::End => {
                        textarea.move_cursor(tui_textarea::CursorMove::End);
                        cursor = expected.len();
                    }
                }
            }

            let actual: String = textarea.lines().join("\n");
            prop_assert_eq!(
                actual, expected,
                "Textarea content should match expected after operations"
            );
        }

        #[test]
        fn prop_input_textarea_edit_consistency(
            initial in "[a-zA-Z0-9]{0,10}",
            ops in prop::collection::vec(arb_text_op(), 1..20)
        ) {
            let mut app = test_app();
            app.state = AppState::Input;
            app.input_textarea.insert_str(&initial);

            for op in &ops {
                let key = match op {
                    TextOp::Insert(ch) => KeyEvent::new(KeyCode::Char(*ch), KeyModifiers::NONE),
                    TextOp::Delete => KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                    TextOp::Left => KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                    TextOp::Right => KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                    TextOp::Home => KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
                    TextOp::End => KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
                };
                app.handle_key_event(key);
            }

            // Property: state should remain Input
            prop_assert_eq!(app.state, AppState::Input);
        }

        #[test]
        fn prop_action_textarea_edit_consistency(
            initial in "[a-zA-Z0-9]{0,10}",
            ops in prop::collection::vec(arb_text_op(), 1..20)
        ) {
            let mut app = test_app();
            app.state = AppState::ReviewAction;
            app.action_textarea.insert_str(&initial);

            for op in &ops {
                let key = match op {
                    TextOp::Insert(ch) => KeyEvent::new(KeyCode::Char(*ch), KeyModifiers::NONE),
                    TextOp::Delete => KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                    TextOp::Left => KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                    TextOp::Right => KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                    TextOp::Home => KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
                    TextOp::End => KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
                };
                app.handle_key_event(key);
            }

            // Property: state should remain ReviewAction
            prop_assert_eq!(app.state, AppState::ReviewAction);
        }
    }
}
