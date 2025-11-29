//! Tool call parsing for AI responses
//!
//! Handles parsing of tool call JSON from AI responses, supporting both
//! raw JSON and markdown code blocks.

use serde::{Deserialize, Serialize};

/// A tool call request from the AI
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The tool to invoke
    pub tool: String,
    /// For run_cmd: the command to execute
    #[serde(default)]
    pub command: String,
    /// For read_file/write_file: the file path
    #[serde(default)]
    pub path: String,
    /// For write_file: the content to write
    #[serde(default)]
    pub content: String,
    /// For search: the pattern to search
    #[serde(default)]
    pub pattern: String,
    /// For search: the directory to search in
    #[serde(default)]
    pub directory: String,
}

impl ToolCall {
    /// Create a new ToolCall
    pub fn new(tool: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            command: command.into(),
            path: String::new(),
            content: String::new(),
            pattern: String::new(),
            directory: String::new(),
        }
    }

    /// Create a run_cmd tool call
    pub fn run_cmd(command: impl Into<String>) -> Self {
        Self::new("run_cmd", command)
    }

    /// Check if this is a run_cmd tool call
    pub fn is_run_cmd(&self) -> bool {
        self.tool == "run_cmd"
    }

    /// Check if this is a read_file tool call
    pub fn is_read_file(&self) -> bool {
        self.tool == "read_file"
    }

    /// Check if this is a write_file tool call
    pub fn is_write_file(&self) -> bool {
        self.tool == "write_file"
    }

    /// Check if this is a search tool call
    pub fn is_search(&self) -> bool {
        self.tool == "search"
    }

    /// Parse AI response for tool call JSON
    ///
    /// Handles both raw JSON and markdown code blocks:
    /// - Raw: `{"tool": "run_cmd", "command": "ls -la"}`
    /// - Markdown: ````json\n{"tool": "run_cmd", "command": "ls -la"}\n````
    ///
    /// Returns `None` if no valid tool call is found.
    pub fn parse(response: &str) -> Option<Self> {
        let trimmed = response.trim();

        // Try parsing as raw JSON first
        if let Some(tool_call) = Self::try_parse_json(trimmed) {
            return Some(tool_call);
        }

        // Try extracting from markdown code blocks
        if let Some(tool_call) = Self::try_parse_markdown_block(trimmed) {
            return Some(tool_call);
        }

        // Try finding JSON object anywhere in the response
        if let Some(tool_call) = Self::try_find_json_object(trimmed) {
            return Some(tool_call);
        }

        None
    }


    /// Try to parse the entire string as JSON
    fn try_parse_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }

    /// Try to extract JSON from markdown code blocks
    ///
    /// Supports:
    /// - ```json ... ```
    /// - ``` ... ```
    fn try_parse_markdown_block(s: &str) -> Option<Self> {
        // Look for ```json or ``` blocks
        let patterns = ["```json", "```"];

        for pattern in patterns {
            if let Some(start_idx) = s.find(pattern) {
                let content_start = start_idx + pattern.len();
                if let Some(end_idx) = s[content_start..].find("```") {
                    let json_content = s[content_start..content_start + end_idx].trim();
                    if let Some(tool_call) = Self::try_parse_json(json_content) {
                        return Some(tool_call);
                    }
                }
            }
        }

        None
    }

    /// Try to find a JSON object anywhere in the response
    ///
    /// Looks for `{...}` patterns and attempts to parse them
    fn try_find_json_object(s: &str) -> Option<Self> {
        let mut depth = 0;
        let mut start: Option<usize> = None;

        for (i, ch) in s.char_indices() {
            match ch {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start_idx) = start {
                            let json_str = &s[start_idx..=i];
                            if let Some(tool_call) = Self::try_parse_json(json_str) {
                                return Some(tool_call);
                            }
                        }
                        start = None;
                    }
                }
                _ => {}
            }
        }

        None
    }
}

/// Result of parsing an AI response
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedResponse {
    /// AI wants to execute a command
    ToolCall(ToolCall),
    /// AI provided a text response (no tool call)
    TextResponse(String),
}

impl ParsedResponse {
    /// Parse an AI response into either a tool call or text response
    pub fn parse(response: &str) -> Self {
        match ToolCall::parse(response) {
            Some(tool_call) => ParsedResponse::ToolCall(tool_call),
            None => ParsedResponse::TextResponse(response.to_string()),
        }
    }

    /// Check if this is a tool call
    pub fn is_tool_call(&self) -> bool {
        matches!(self, ParsedResponse::ToolCall(_))
    }

    /// Check if this is a text response
    pub fn is_text_response(&self) -> bool {
        matches!(self, ParsedResponse::TextResponse(_))
    }

    /// Get the tool call if this is one
    pub fn as_tool_call(&self) -> Option<&ToolCall> {
        match self {
            ParsedResponse::ToolCall(tc) => Some(tc),
            _ => None,
        }
    }

    /// Get the text response if this is one
    pub fn as_text_response(&self) -> Option<&str> {
        match self {
            ParsedResponse::TextResponse(s) => Some(s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Strategy to generate valid tool names (non-empty alphanumeric with underscores)
    fn arb_tool_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,20}".prop_map(|s| s)
    }

    // Strategy to generate valid command strings (non-empty, printable ASCII)
    fn arb_command() -> impl Strategy<Value = String> {
        // Generate commands that are valid shell-like strings
        "[a-zA-Z][a-zA-Z0-9 _\\-./]{0,100}".prop_map(|s| s)
    }

    // Strategy to generate valid ToolCall instances
    fn arb_tool_call() -> impl Strategy<Value = ToolCall> {
        (arb_tool_name(), arb_command()).prop_map(|(tool, command)| ToolCall::new(tool, command))
    }

    // **Feature: agent-rs, Property 3: Tool Call Parsing Round-Trip**
    // *For any* valid ToolCall struct, serializing it to JSON and parsing it back
    // SHALL produce an equivalent ToolCall.
    // **Validates: Requirements 2.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_tool_call_json_roundtrip(tool_call in arb_tool_call()) {
            // Serialize to JSON
            let json = serde_json::to_string(&tool_call).unwrap();
            
            // Parse back using our parse function
            let parsed = ToolCall::parse(&json);
            
            // Property: parsing should succeed
            prop_assert!(parsed.is_some(), "Failed to parse serialized ToolCall: {}", json);
            
            let recovered = parsed.unwrap();
            
            // Property: recovered ToolCall should equal original
            prop_assert_eq!(tool_call.tool, recovered.tool, "Tool name mismatch");
            prop_assert_eq!(tool_call.command, recovered.command, "Command mismatch");
        }

        #[test]
        fn prop_tool_call_serde_roundtrip(tool_call in arb_tool_call()) {
            // Serialize to JSON
            let json = serde_json::to_string(&tool_call).unwrap();
            
            // Deserialize back using serde directly
            let recovered: ToolCall = serde_json::from_str(&json).unwrap();
            
            // Property: should be identical
            prop_assert_eq!(tool_call, recovered);
        }

        #[test]
        fn prop_tool_call_markdown_roundtrip(tool_call in arb_tool_call()) {
            // Serialize to JSON and wrap in markdown code block
            let json = serde_json::to_string(&tool_call).unwrap();
            let markdown = format!("```json\n{}\n```", json);
            
            // Parse from markdown
            let parsed = ToolCall::parse(&markdown);
            
            // Property: parsing should succeed
            prop_assert!(parsed.is_some(), "Failed to parse markdown-wrapped ToolCall");
            
            let recovered = parsed.unwrap();
            
            // Property: recovered ToolCall should equal original
            prop_assert_eq!(tool_call.tool, recovered.tool);
            prop_assert_eq!(tool_call.command, recovered.command);
        }

        #[test]
        fn prop_tool_call_embedded_roundtrip(tool_call in arb_tool_call()) {
            // Serialize to JSON and embed in text
            let json = serde_json::to_string(&tool_call).unwrap();
            let embedded = format!("Here's the command: {}", json);
            
            // Parse from embedded text
            let parsed = ToolCall::parse(&embedded);
            
            // Property: parsing should succeed
            prop_assert!(parsed.is_some(), "Failed to parse embedded ToolCall");
            
            let recovered = parsed.unwrap();
            
            // Property: recovered ToolCall should equal original
            prop_assert_eq!(tool_call.tool, recovered.tool);
            prop_assert_eq!(tool_call.command, recovered.command);
        }
    }

    // Strategy to generate plain text responses (no valid JSON tool calls)
    // These are strings that should NOT be parsed as tool calls
    fn arb_plain_text() -> impl Strategy<Value = String> {
        // Generate text that doesn't contain valid tool call JSON
        // Avoid generating strings that could accidentally be valid JSON
        prop_oneof![
            // Simple text responses
            "[A-Za-z][A-Za-z0-9 ,.!?]{0,200}",
            // Numbers and simple answers
            "[0-9]{1,10}",
            // Text with special characters but no JSON
            "[A-Za-z ]+[.!?]",
        ]
    }

    // **Feature: agent-rs, Property 4: Non-Tool Response Handling**
    // *For any* AI response string that does not contain valid tool call JSON,
    // the response SHALL be classified as TextResponse and the original text preserved.
    // **Validates: Requirements 2.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_non_tool_response_classified_as_text(text in arb_plain_text()) {
            // Parse the plain text response
            let parsed = ParsedResponse::parse(&text);
            
            // Property: should be classified as TextResponse (not ToolCall)
            prop_assert!(
                parsed.is_text_response(),
                "Plain text '{}' should be classified as TextResponse, got {:?}",
                text,
                parsed
            );
            
            // Property: original text should be preserved
            prop_assert_eq!(
                parsed.as_text_response().unwrap(),
                &text,
                "Original text should be preserved"
            );
        }

        #[test]
        fn prop_invalid_json_classified_as_text(text in arb_plain_text()) {
            // Create invalid JSON-like strings
            let invalid_json = format!("{{\"invalid\": {}}}", text);
            
            // Parse the invalid JSON
            let parsed = ParsedResponse::parse(&invalid_json);
            
            // Property: invalid JSON should be classified as TextResponse
            prop_assert!(
                parsed.is_text_response(),
                "Invalid JSON '{}' should be classified as TextResponse",
                invalid_json
            );
        }

        #[test]
        fn prop_tool_call_classified_correctly(tool_call in arb_tool_call()) {
            // Serialize to JSON
            let json = serde_json::to_string(&tool_call).unwrap();
            
            // Parse the response
            let parsed = ParsedResponse::parse(&json);
            
            // Property: should be classified as ToolCall
            prop_assert!(
                parsed.is_tool_call(),
                "Valid tool call JSON should be classified as ToolCall"
            );
            
            // Property: tool call should match original
            let recovered = parsed.as_tool_call().unwrap();
            prop_assert_eq!(&tool_call.tool, &recovered.tool);
            prop_assert_eq!(&tool_call.command, &recovered.command);
        }

        #[test]
        fn prop_text_response_preserves_content(text in arb_plain_text()) {
            // Create a TextResponse directly
            let response = ParsedResponse::TextResponse(text.clone());
            
            // Property: as_text_response should return the original text
            prop_assert_eq!(response.as_text_response(), Some(text.as_str()));
            
            // Property: as_tool_call should return None
            prop_assert!(response.as_tool_call().is_none());
        }
    }

    #[test]
    fn test_parse_raw_json() {
        let response = r#"{"tool": "run_cmd", "command": "ls -la"}"#;
        let tool_call = ToolCall::parse(response).unwrap();
        assert_eq!(tool_call.tool, "run_cmd");
        assert_eq!(tool_call.command, "ls -la");
    }

    #[test]
    fn test_parse_raw_json_with_whitespace() {
        let response = r#"  {"tool": "run_cmd", "command": "pwd"}  "#;
        let tool_call = ToolCall::parse(response).unwrap();
        assert_eq!(tool_call.tool, "run_cmd");
        assert_eq!(tool_call.command, "pwd");
    }

    #[test]
    fn test_parse_markdown_json_block() {
        let response = r#"```json
{"tool": "run_cmd", "command": "echo hello"}
```"#;
        let tool_call = ToolCall::parse(response).unwrap();
        assert_eq!(tool_call.tool, "run_cmd");
        assert_eq!(tool_call.command, "echo hello");
    }

    #[test]
    fn test_parse_markdown_block_no_lang() {
        let response = r#"```
{"tool": "run_cmd", "command": "cat file.txt"}
```"#;
        let tool_call = ToolCall::parse(response).unwrap();
        assert_eq!(tool_call.tool, "run_cmd");
        assert_eq!(tool_call.command, "cat file.txt");
    }

    #[test]
    fn test_parse_embedded_json() {
        let response = r#"I'll run this command for you: {"tool": "run_cmd", "command": "df -h"}"#;
        let tool_call = ToolCall::parse(response).unwrap();
        assert_eq!(tool_call.tool, "run_cmd");
        assert_eq!(tool_call.command, "df -h");
    }

    #[test]
    fn test_parse_no_tool_call() {
        let response = "The answer is 42.";
        assert!(ToolCall::parse(response).is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        let response = r#"{"tool": "run_cmd", "command": }"#;
        assert!(ToolCall::parse(response).is_none());
    }

    #[test]
    fn test_parse_wrong_json_structure() {
        let response = r#"{"name": "test", "value": 123}"#;
        assert!(ToolCall::parse(response).is_none());
    }

    #[test]
    fn test_parsed_response_tool_call() {
        let response = r#"{"tool": "run_cmd", "command": "ls"}"#;
        let parsed = ParsedResponse::parse(response);
        assert!(parsed.is_tool_call());
        assert!(!parsed.is_text_response());
        assert_eq!(parsed.as_tool_call().unwrap().command, "ls");
    }

    #[test]
    fn test_parsed_response_text() {
        let response = "Hello, how can I help you?";
        let parsed = ParsedResponse::parse(response);
        assert!(parsed.is_text_response());
        assert!(!parsed.is_tool_call());
        assert_eq!(parsed.as_text_response().unwrap(), response);
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall::run_cmd("ls -la");
        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains("run_cmd"));
        assert!(json.contains("ls -la"));
    }

    #[test]
    fn test_is_run_cmd() {
        let tool_call = ToolCall::run_cmd("test");
        assert!(tool_call.is_run_cmd());

        let other = ToolCall::new("other_tool", "test");
        assert!(!other.is_run_cmd());
    }
}
