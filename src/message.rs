//! Message types for conversation history
//!
//! Defines Message and MessageRole for AI conversation tracking,
//! with serialization support for the Gemini API format.

use serde::{Deserialize, Serialize};

/// Role of a message in the conversation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// User input
    User,
    /// AI model response
    Model,
    /// System instructions (not sent as regular content)
    System,
}

/// A single message in the conversation history
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
}

impl Message {
    /// Create a new message
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create a model message
    pub fn model(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Model, content)
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }
}

// Gemini API types for serialization

/// Gemini API request format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiRequest {
    /// Conversation contents
    pub contents: Vec<GeminiContent>,
    /// System instruction (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
}

/// Gemini content block
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeminiContent {
    /// Role: "user" or "model"
    pub role: String,
    /// Parts of the content
    pub parts: Vec<GeminiPart>,
}

/// Gemini content part
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeminiPart {
    /// Text content
    pub text: String,
}

/// Gemini system instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiSystemInstruction {
    /// Parts of the system instruction
    pub parts: Vec<GeminiPart>,
}

/// Gemini API response format
#[derive(Debug, Clone, Deserialize)]
pub struct GeminiResponse {
    /// Response candidates
    pub candidates: Vec<GeminiCandidate>,
}

/// Gemini response candidate
#[derive(Debug, Clone, Deserialize)]
pub struct GeminiCandidate {
    /// Content of the response
    pub content: GeminiContent,
}

impl Message {
    /// Convert to Gemini API content format
    pub fn to_gemini_content(&self) -> GeminiContent {
        GeminiContent {
            role: match self.role {
                MessageRole::User => "user".to_string(),
                MessageRole::Model => "model".to_string(),
                MessageRole::System => "user".to_string(), // System messages handled separately
            },
            parts: vec![GeminiPart {
                text: self.content.clone(),
            }],
        }
    }

    /// Create from Gemini API content format
    pub fn from_gemini_content(content: &GeminiContent) -> Self {
        let role = match content.role.as_str() {
            "user" => MessageRole::User,
            "model" => MessageRole::Model,
            _ => MessageRole::Model, // Default to model for unknown roles
        };
        let text = content
            .parts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("");
        Self::new(role, text)
    }
}

/// Convert a slice of messages to Gemini API request format
pub fn messages_to_gemini_request(messages: &[Message]) -> GeminiRequest {
    let mut system_instruction = None;
    let mut contents = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {
                // System messages become system_instruction
                system_instruction = Some(GeminiSystemInstruction {
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                    }],
                });
            }
            _ => {
                contents.push(msg.to_gemini_content());
            }
        }
    }

    GeminiRequest {
        contents,
        system_instruction,
    }
}

/// Convert Gemini API response to messages
pub fn gemini_response_to_messages(response: &GeminiResponse) -> Vec<Message> {
    response
        .candidates
        .iter()
        .map(|c| Message::from_gemini_content(&c.content))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");

        let msg = Message::model("Hi there");
        assert_eq!(msg.role, MessageRole::Model);
        assert_eq!(msg.content, "Hi there");
    }

    #[test]
    fn test_to_gemini_content() {
        let msg = Message::user("Test message");
        let content = msg.to_gemini_content();
        assert_eq!(content.role, "user");
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts[0].text, "Test message");
    }

    #[test]
    fn test_from_gemini_content() {
        let content = GeminiContent {
            role: "model".to_string(),
            parts: vec![GeminiPart {
                text: "Response text".to_string(),
            }],
        };
        let msg = Message::from_gemini_content(&content);
        assert_eq!(msg.role, MessageRole::Model);
        assert_eq!(msg.content, "Response text");
    }

    #[test]
    fn test_messages_to_gemini_request() {
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::user("Hello"),
            Message::model("Hi!"),
        ];
        let request = messages_to_gemini_request(&messages);
        
        assert!(request.system_instruction.is_some());
        assert_eq!(request.contents.len(), 2);
        assert_eq!(request.contents[0].role, "user");
        assert_eq!(request.contents[1].role, "model");
    }

    // Strategy to generate arbitrary MessageRole (excluding System for round-trip)
    fn arb_message_role() -> impl Strategy<Value = MessageRole> {
        prop_oneof![
            Just(MessageRole::User),
            Just(MessageRole::Model),
        ]
    }

    // Strategy to generate arbitrary Message
    fn arb_message() -> impl Strategy<Value = Message> {
        (arb_message_role(), ".*")
            .prop_map(|(role, content)| Message::new(role, content))
    }

    // Strategy to generate a vector of messages (alternating user/model for valid conversation)
    fn arb_conversation() -> impl Strategy<Value = Vec<Message>> {
        prop::collection::vec(arb_message(), 0..20)
    }

    // **Feature: agent-rs, Property 13: History Serialization Round-Trip**
    // *For any* valid Vec<Message>, serializing it to Gemini API format and
    // deserializing back SHALL preserve the message count, roles, and content.
    // **Validates: Requirements 6.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_message_gemini_roundtrip(msg in arb_message()) {
            // Convert to Gemini format and back
            let gemini_content = msg.to_gemini_content();
            let recovered = Message::from_gemini_content(&gemini_content);

            // Role and content should be preserved
            prop_assert_eq!(msg.role, recovered.role);
            prop_assert_eq!(msg.content, recovered.content);
        }

        #[test]
        fn prop_conversation_serialization_preserves_count(messages in arb_conversation()) {
            // Filter out system messages for this test (they're handled separately)
            let non_system: Vec<_> = messages.iter()
                .filter(|m| m.role != MessageRole::System)
                .cloned()
                .collect();

            let request = messages_to_gemini_request(&non_system);

            // Content count should match non-system message count
            prop_assert_eq!(request.contents.len(), non_system.len());
        }

        #[test]
        fn prop_message_json_roundtrip(msg in arb_message()) {
            // Serialize to JSON and back
            let json = serde_json::to_string(&msg).unwrap();
            let recovered: Message = serde_json::from_str(&json).unwrap();

            // Should be identical
            prop_assert_eq!(msg, recovered);
        }

        #[test]
        fn prop_gemini_content_json_roundtrip(msg in arb_message()) {
            // Convert to Gemini content, serialize to JSON, deserialize, convert back
            let gemini_content = msg.to_gemini_content();
            let json = serde_json::to_string(&gemini_content).unwrap();
            let recovered_content: GeminiContent = serde_json::from_str(&json).unwrap();
            let recovered_msg = Message::from_gemini_content(&recovered_content);

            // Role and content should be preserved
            prop_assert_eq!(msg.role, recovered_msg.role);
            prop_assert_eq!(msg.content, recovered_msg.content);
        }
    }
}
