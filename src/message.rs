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

/// Image data for multimodal messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageData {
    /// Base64 encoded image data
    pub base64: String,
    /// MIME type (e.g., "image/png", "image/jpeg")
    pub mime_type: String,
}

/// A single message in the conversation history
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
    /// Optional image attachment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageData>,
}

impl Message {
    /// Create a new message
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            image: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create a user message with image
    pub fn user_with_image(content: impl Into<String>, image: ImageData) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            image: Some(image),
        }
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

impl ImageData {
    /// Load image from file path
    pub fn from_file(path: &str) -> std::io::Result<Self> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let base64 = base64_encode(&buffer);
        let mime_type = mime_from_path(path);

        Ok(Self { base64, mime_type })
    }
}

fn base64_encode(data: &[u8]) -> String {
    use std::io::Write;
    let mut enc = Vec::new();
    let mut encoder = Base64Encoder::new(&mut enc);
    encoder.write_all(data).unwrap();
    drop(encoder);
    String::from_utf8(enc).unwrap()
}

// Simple base64 encoder
struct Base64Encoder<W: std::io::Write> {
    writer: W,
    buf: [u8; 3],
    buf_len: usize,
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

impl<W: std::io::Write> Base64Encoder<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            buf: [0; 3],
            buf_len: 0,
        }
    }

    fn encode_block(&mut self) -> std::io::Result<()> {
        let b = &self.buf;
        let out = [
            BASE64_CHARS[(b[0] >> 2) as usize],
            BASE64_CHARS[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize],
            if self.buf_len > 1 {
                BASE64_CHARS[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize]
            } else {
                b'='
            },
            if self.buf_len > 2 {
                BASE64_CHARS[(b[2] & 0x3f) as usize]
            } else {
                b'='
            },
        ];
        self.writer.write_all(&out)
    }
}

impl<W: std::io::Write> std::io::Write for Base64Encoder<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        for &byte in data {
            self.buf[self.buf_len] = byte;
            self.buf_len += 1;
            if self.buf_len == 3 {
                self.encode_block()?;
                self.buf_len = 0;
                self.buf = [0; 3];
            }
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<W: std::io::Write> Drop for Base64Encoder<W> {
    fn drop(&mut self) {
        if self.buf_len > 0 {
            let _ = self.encode_block();
        }
    }
}

fn mime_from_path(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
    .to_string()
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

/// Gemini content part (text or image)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeminiPart {
    Text { text: String },
    Image { inline_data: GeminiInlineData },
}

/// Inline image data for Gemini API
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeminiInlineData {
    pub mime_type: String,
    pub data: String,
}

impl GeminiPart {
    pub fn text(s: impl Into<String>) -> Self {
        GeminiPart::Text { text: s.into() }
    }

    pub fn image(mime_type: String, base64_data: String) -> Self {
        GeminiPart::Image {
            inline_data: GeminiInlineData {
                mime_type,
                data: base64_data,
            },
        }
    }
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
        let mut parts = vec![GeminiPart::text(&self.content)];
        if let Some(ref img) = self.image {
            parts.push(GeminiPart::image(img.mime_type.clone(), img.base64.clone()));
        }
        GeminiContent {
            role: match self.role {
                MessageRole::User => "user".to_string(),
                MessageRole::Model => "model".to_string(),
                MessageRole::System => "user".to_string(),
            },
            parts,
        }
    }

    /// Create from Gemini API content format
    pub fn from_gemini_content(content: &GeminiContent) -> Self {
        let role = match content.role.as_str() {
            "user" => MessageRole::User,
            "model" => MessageRole::Model,
            _ => MessageRole::Model,
        };
        let text = content
            .parts
            .iter()
            .filter_map(|p| match p {
                GeminiPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
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
                system_instruction = Some(GeminiSystemInstruction {
                    parts: vec![GeminiPart::text(&msg.content)],
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
        prop_oneof![Just(MessageRole::User), Just(MessageRole::Model),]
    }

    // Strategy to generate arbitrary Message
    fn arb_message() -> impl Strategy<Value = Message> {
        (arb_message_role(), ".*").prop_map(|(role, content)| Message::new(role, content))
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
