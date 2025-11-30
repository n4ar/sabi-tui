//! OpenAI-compatible API client

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::Config;
use crate::gemini::SYSTEM_PROMPT;
use crate::message::{Message, MessageRole};

#[derive(Debug, Error)]
pub enum OpenAIError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },
    #[error("Missing API key")]
    MissingApiKey,
    #[error("Empty response")]
    EmptyResponse,
}

#[derive(Clone)]
pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    max_history_messages: usize,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

impl OpenAIClient {
    pub fn new(config: &Config) -> Result<Self, OpenAIError> {
        if config.api_key.is_empty() {
            return Err(OpenAIError::MissingApiKey);
        }

        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        Ok(Self {
            client: Client::new(),
            api_key: config.api_key.clone(),
            base_url,
            model: config.model.clone(),
            max_history_messages: config.max_history_messages,
        })
    }

    pub async fn chat(&self, messages: &[Message]) -> Result<String, OpenAIError> {
        let url = format!("{}/chat/completions", self.base_url);

        // Build messages with system prompt
        let mut chat_messages = vec![ChatMessage {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string(),
        }];

        // Add conversation history (sliding window)
        let start = messages.len().saturating_sub(self.max_history_messages);
        for msg in &messages[start..] {
            if msg.role == MessageRole::System {
                continue;
            }
            chat_messages.push(ChatMessage {
                role: match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Model => "assistant",
                    MessageRole::System => "system",
                }
                .to_string(),
                content: msg.content.clone(),
            });
        }

        let request = ChatRequest {
            model: self.model.clone(),
            messages: chat_messages,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(OpenAIError::ApiError { status, message });
        }

        let body: ChatResponse = response.json().await?;
        body.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or(OpenAIError::EmptyResponse)
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}
