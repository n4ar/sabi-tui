//! Core application struct
//!
//! Contains the App struct that holds all application state.

use crate::config::Config;
use crate::message::Message;
use crate::state::AppState;

/// Main application state container
pub struct App {
    /// Current application state
    pub state: AppState,

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

impl App {
    /// Create a new App instance with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            state: AppState::default(),
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
}
