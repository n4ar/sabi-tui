//! Gemini API integration
//!
//! Handles communication with the Gemini API for AI-powered command generation.

use reqwest::Client;
use thiserror::Error;

use crate::config::Config;
use crate::message::{
    GeminiCandidate, GeminiContent, GeminiPart, GeminiRequest, GeminiResponse,
    GeminiSystemInstruction, Message, MessageRole,
};

/// System prompt defining the AI's behavior as a system expert
pub const SYSTEM_PROMPT: &str = r#"
You are a macOS/Linux system expert assistant. You help users accomplish system administration tasks.

You have access to these tools (respond with JSON):

1. Run shell command:
   {"tool": "run_cmd", "command": "<shell command>"}

2. Read file contents:
   {"tool": "read_file", "path": "<file path>"}

3. Write to file:
   {"tool": "write_file", "path": "<file path>", "content": "<content>"}

4. Search for files:
   {"tool": "search", "pattern": "<filename pattern>", "directory": "<dir>"}

Rules:
1. Only output raw JSON when using a tool. No markdown, no explanation before it.
2. If you can answer without a tool, just respond with plain text.
3. After seeing tool output, provide a helpful summary.
4. Be concise but informative.
5. For dangerous operations, warn the user first.

Examples:
- "List files": {"tool": "run_cmd", "command": "ls -la"}
- "Show config.toml": {"tool": "read_file", "path": "config.toml"}
- "Find all .rs files": {"tool": "search", "pattern": "*.rs", "directory": "."}
- "What is 2+2?": 4
"#;

/// Errors that can occur during Gemini API operations
#[derive(Debug, Error)]
pub enum GeminiError {
    /// Network or HTTP error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded. Please wait and try again.")]
    RateLimited,

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Missing API key
    #[error("Missing API key. Set AGENT_RS_API_KEY or configure in config.toml")]
    MissingApiKey,

    /// Empty response from API
    #[error("Empty response from API")]
    EmptyResponse,
}


/// Client for interacting with the Gemini API
#[derive(Clone)]
pub struct GeminiClient {
    /// HTTP client for making requests
    client: Client,
    /// API key for authentication
    api_key: String,
    /// Model name to use
    model: String,
    /// Maximum messages to keep in history (sliding window)
    max_history_messages: usize,
}

impl GeminiClient {
    /// Create a new GeminiClient from configuration
    pub fn new(config: &Config) -> Result<Self, GeminiError> {
        if config.api_key.is_empty() {
            return Err(GeminiError::MissingApiKey);
        }

        Ok(Self {
            client: Client::new(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            max_history_messages: config.max_history_messages,
        })
    }

    /// Create a GeminiClient with custom parameters (for testing)
    pub fn with_params(api_key: String, model: String, max_history_messages: usize) -> Result<Self, GeminiError> {
        if api_key.is_empty() {
            return Err(GeminiError::MissingApiKey);
        }

        Ok(Self {
            client: Client::new(),
            api_key,
            model,
            max_history_messages,
        })
    }

    /// Send a conversation to the Gemini API and get a response
    ///
    /// This method applies a sliding window to keep the conversation within limits,
    /// always preserving the system prompt if present.
    pub async fn chat(&self, messages: &[Message]) -> Result<String, GeminiError> {
        let windowed_messages = self.apply_sliding_window(messages);
        let request = self.build_request(&windowed_messages);

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(GeminiError::RateLimited);
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(GeminiError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let gemini_response: GeminiResponse = response.json().await.map_err(|e| {
            GeminiError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        self.extract_text(&gemini_response)
    }

    /// Apply sliding window to keep conversation within limits
    ///
    /// Always keeps the system prompt (if present) plus the most recent messages
    /// up to max_history_messages.
    pub fn apply_sliding_window<'a>(&self, messages: &'a [Message]) -> Vec<&'a Message> {
        let mut result = Vec::new();
        let mut system_prompt: Option<&Message> = None;
        let mut non_system: Vec<&Message> = Vec::new();

        // Separate system prompt from other messages
        for msg in messages {
            if msg.role == MessageRole::System {
                system_prompt = Some(msg);
            } else {
                non_system.push(msg);
            }
        }

        // Always include system prompt first if present
        if let Some(sys) = system_prompt {
            result.push(sys);
        }

        // Apply sliding window to non-system messages
        let window_size = self.max_history_messages;
        if non_system.len() > window_size {
            let start = non_system.len() - window_size;
            result.extend(&non_system[start..]);
        } else {
            result.extend(non_system);
        }

        result
    }

    /// Build a Gemini API request from messages
    fn build_request(&self, messages: &[&Message]) -> GeminiRequest {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    system_instruction = Some(GeminiSystemInstruction {
                        parts: vec![GeminiPart {
                            text: msg.content.clone(),
                        }],
                    });
                }
                _ => {
                    contents.push(GeminiContent {
                        role: match msg.role {
                            MessageRole::User => "user".to_string(),
                            MessageRole::Model => "model".to_string(),
                            MessageRole::System => "user".to_string(),
                        },
                        parts: vec![GeminiPart {
                            text: msg.content.clone(),
                        }],
                    });
                }
            }
        }

        GeminiRequest {
            contents,
            system_instruction,
        }
    }

    /// Extract text content from Gemini API response
    fn extract_text(&self, response: &GeminiResponse) -> Result<String, GeminiError> {
        let candidate = response
            .candidates
            .first()
            .ok_or(GeminiError::EmptyResponse)?;

        let text = candidate
            .content
            .parts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            return Err(GeminiError::EmptyResponse);
        }

        Ok(text)
    }

    /// Get the maximum history messages setting
    pub fn max_history_messages(&self) -> usize {
        self.max_history_messages
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Strategy to generate arbitrary MessageRole
    fn arb_message_role() -> impl Strategy<Value = MessageRole> {
        prop_oneof![Just(MessageRole::User), Just(MessageRole::Model),]
    }

    // Strategy to generate arbitrary Message (non-system)
    fn arb_message() -> impl Strategy<Value = Message> {
        (arb_message_role(), "[a-zA-Z0-9 ]{1,50}")
            .prop_map(|(role, content)| Message::new(role, content))
    }

    // Strategy to generate a conversation with optional system prompt
    fn arb_conversation_with_system() -> impl Strategy<Value = (Option<Message>, Vec<Message>)> {
        (
            prop::option::of("[a-zA-Z0-9 ]{10,100}".prop_map(Message::system)),
            prop::collection::vec(arb_message(), 0..50),
        )
    }

    // **Feature: agent-rs, Property 21: Sliding Window History Management**
    // *For any* conversation with more than max_history_messages, the API request
    // SHALL include only the system prompt plus the most recent max_history_messages,
    // preserving conversation continuity.
    // **Validates: Requirements 6.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_sliding_window_preserves_system_prompt(
            (system_msg, non_system) in arb_conversation_with_system(),
            max_history in 1usize..20,
        ) {
            // Build messages list
            let mut messages = Vec::new();
            if let Some(ref sys) = system_msg {
                messages.push(sys.clone());
            }
            messages.extend(non_system.clone());

            // Create client with specific max_history
            let client = GeminiClient {
                client: Client::new(),
                api_key: "test-key".to_string(),
                model: "test-model".to_string(),
                max_history_messages: max_history,
            };

            let windowed = client.apply_sliding_window(&messages);

            // Property: If system prompt exists, it should be first in result
            if system_msg.is_some() {
                prop_assert!(!windowed.is_empty());
                prop_assert_eq!(windowed[0].role.clone(), MessageRole::System);
            }
        }

        #[test]
        fn prop_sliding_window_limits_non_system_messages(
            (system_msg, non_system) in arb_conversation_with_system(),
            max_history in 1usize..20,
        ) {
            // Build messages list
            let mut messages = Vec::new();
            if let Some(ref sys) = system_msg {
                messages.push(sys.clone());
            }
            messages.extend(non_system.clone());

            let client = GeminiClient {
                client: Client::new(),
                api_key: "test-key".to_string(),
                model: "test-model".to_string(),
                max_history_messages: max_history,
            };

            let windowed = client.apply_sliding_window(&messages);

            // Count non-system messages in result
            let non_system_count = windowed.iter()
                .filter(|m| m.role != MessageRole::System)
                .count();

            // Property: Non-system messages should be at most max_history
            prop_assert!(
                non_system_count <= max_history,
                "Expected at most {} non-system messages, got {}",
                max_history,
                non_system_count
            );
        }

        #[test]
        fn prop_sliding_window_keeps_most_recent(
            (system_msg, non_system) in arb_conversation_with_system(),
            max_history in 1usize..20,
        ) {
            // Build messages list
            let mut messages = Vec::new();
            if let Some(ref sys) = system_msg {
                messages.push(sys.clone());
            }
            messages.extend(non_system.clone());

            let client = GeminiClient {
                client: Client::new(),
                api_key: "test-key".to_string(),
                model: "test-model".to_string(),
                max_history_messages: max_history,
            };

            let windowed = client.apply_sliding_window(&messages);

            // Get non-system messages from result
            let windowed_non_system: Vec<_> = windowed.iter()
                .filter(|m| m.role != MessageRole::System)
                .collect();

            // Property: If we have more messages than window, we should have the most recent ones
            if non_system.len() > max_history {
                let expected_start = non_system.len() - max_history;
                for (i, msg) in windowed_non_system.iter().enumerate() {
                    prop_assert_eq!(
                        &msg.content,
                        &non_system[expected_start + i].content
                    );
                }
            }
        }

        #[test]
        fn prop_sliding_window_preserves_all_when_under_limit(
            (system_msg, non_system) in arb_conversation_with_system(),
        ) {
            // Use a large max_history to ensure we're under the limit
            let max_history = non_system.len() + 10;

            // Build messages list
            let mut messages = Vec::new();
            if let Some(ref sys) = system_msg {
                messages.push(sys.clone());
            }
            messages.extend(non_system.clone());

            let client = GeminiClient {
                client: Client::new(),
                api_key: "test-key".to_string(),
                model: "test-model".to_string(),
                max_history_messages: max_history,
            };

            let windowed = client.apply_sliding_window(&messages);

            // Count non-system messages
            let windowed_non_system_count = windowed.iter()
                .filter(|m| m.role != MessageRole::System)
                .count();

            // Property: All non-system messages should be preserved when under limit
            prop_assert_eq!(
                windowed_non_system_count,
                non_system.len(),
                "All messages should be preserved when under limit"
            );
        }
    }

    #[test]
    fn test_sliding_window_basic() {
        let client = GeminiClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: "test".to_string(),
            max_history_messages: 3,
        };

        let messages = vec![
            Message::system("System prompt"),
            Message::user("First"),
            Message::model("Response 1"),
            Message::user("Second"),
            Message::model("Response 2"),
            Message::user("Third"),
        ];

        let windowed = client.apply_sliding_window(&messages);

        // Should have system + last 3 messages
        // Non-system messages in order: First, Response 1, Second, Response 2, Third
        // Last 3: Second, Response 2, Third
        assert_eq!(windowed.len(), 4);
        assert_eq!(windowed[0].role, MessageRole::System);
        assert_eq!(windowed[1].content, "Second");
        assert_eq!(windowed[2].content, "Response 2");
        assert_eq!(windowed[3].content, "Third");
    }

    #[test]
    fn test_sliding_window_no_system() {
        let client = GeminiClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: "test".to_string(),
            max_history_messages: 2,
        };

        let messages = vec![
            Message::user("First"),
            Message::model("Response 1"),
            Message::user("Second"),
        ];

        let windowed = client.apply_sliding_window(&messages);

        // Should have last 2 messages only
        assert_eq!(windowed.len(), 2);
        assert_eq!(windowed[0].content, "Response 1");
        assert_eq!(windowed[1].content, "Second");
    }

    #[test]
    fn test_missing_api_key_error() {
        let config = Config {
            api_key: String::new(),
            ..Config::default()
        };

        let result = GeminiClient::new(&config);
        assert!(matches!(result, Err(GeminiError::MissingApiKey)));
    }

    #[test]
    fn test_build_request_with_system() {
        let client = GeminiClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: "test".to_string(),
            max_history_messages: 10,
        };

        let messages = vec![
            Message::system("Be helpful"),
            Message::user("Hello"),
        ];

        let refs: Vec<&Message> = messages.iter().collect();
        let request = client.build_request(&refs);

        assert!(request.system_instruction.is_some());
        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.contents[0].role, "user");
    }

    #[test]
    fn test_gemini_error_display() {
        // Test error message formatting
        let network_err = GeminiError::MissingApiKey;
        assert!(network_err.to_string().contains("Missing API key"));

        let api_err = GeminiError::ApiError {
            status: 400,
            message: "Bad request".to_string(),
        };
        assert!(api_err.to_string().contains("400"));
        assert!(api_err.to_string().contains("Bad request"));

        let rate_err = GeminiError::RateLimited;
        assert!(rate_err.to_string().contains("Rate limit"));

        let invalid_err = GeminiError::InvalidResponse("parse error".to_string());
        assert!(invalid_err.to_string().contains("parse error"));

        let empty_err = GeminiError::EmptyResponse;
        assert!(empty_err.to_string().contains("Empty response"));
    }

    #[test]
    fn test_extract_text_empty_candidates() {
        let client = GeminiClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: "test".to_string(),
            max_history_messages: 10,
        };

        let response = GeminiResponse {
            candidates: vec![],
        };

        let result = client.extract_text(&response);
        assert!(matches!(result, Err(GeminiError::EmptyResponse)));
    }

    #[test]
    fn test_extract_text_empty_content() {
        let client = GeminiClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: "test".to_string(),
            max_history_messages: 10,
        };

        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: "".to_string(),
                    }],
                },
            }],
        };

        let result = client.extract_text(&response);
        assert!(matches!(result, Err(GeminiError::EmptyResponse)));
    }

    #[test]
    fn test_extract_text_success() {
        let client = GeminiClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: "test".to_string(),
            max_history_messages: 10,
        };

        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: "Hello, world!".to_string(),
                    }],
                },
            }],
        };

        let result = client.extract_text(&response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world!");
    }
}
