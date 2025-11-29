//! Configuration management
//!
//! Handles loading configuration from files and environment variables.

use serde::Deserialize;
use thiserror::Error;
use std::path::PathBuf;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Config file not found
    #[error("Config file not found")]
    NotFound,

    /// Invalid config format
    #[error("Invalid config format: {0}")]
    InvalidFormat(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse error
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
}

/// Application configuration
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Config {
    /// Gemini API key
    #[serde(default)]
    pub api_key: String,

    /// Model name
    #[serde(default = "default_model")]
    pub model: String,

    /// Maximum messages to keep in history
    #[serde(default = "default_max_history")]
    pub max_history_messages: usize,

    /// Maximum output bytes to capture
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,

    /// Maximum output lines to capture
    #[serde(default = "default_max_output_lines")]
    pub max_output_lines: usize,

    /// Dangerous command patterns
    #[serde(default = "default_dangerous_patterns")]
    pub dangerous_patterns: Vec<String>,
}

fn default_model() -> String {
    "gemini-2.5-flash".to_string()
}

fn default_max_history() -> usize {
    20
}

fn default_max_output_bytes() -> usize {
    50 * 1024 // 50KB
}

fn default_max_output_lines() -> usize {
    500
}

fn default_dangerous_patterns() -> Vec<String> {
    vec![
        r"rm\s+-rf\s+/".to_string(),
        r"mkfs".to_string(),
        r"dd\s+if=".to_string(),
        r":\(\)\s*\{".to_string(), // fork bomb
        r">\s*/dev/sd".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_model(),
            max_history_messages: default_max_history(),
            max_output_bytes: default_max_output_bytes(),
            max_output_lines: default_max_output_lines(),
            dangerous_patterns: default_dangerous_patterns(),
        }
    }
}

impl Config {
    /// Load configuration from file and environment variables
    ///
    /// Precedence (highest to lowest):
    /// 1. Environment variables (AGENT_RS_API_KEY, etc.)
    /// 2. Config file (~/.config/agent-rs/config.toml)
    /// 3. Default values
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Self::load_from_file().unwrap_or_default();
        config.apply_env_overrides();
        Ok(config)
    }

    /// Load configuration with a custom config path (for testing)
    pub fn load_with_path(config_path: Option<&PathBuf>) -> Result<Self, ConfigError> {
        let mut config = match config_path {
            Some(path) if path.exists() => {
                let content = std::fs::read_to_string(path)?;
                toml::from_str(&content)?
            }
            _ => Self::default(),
        };
        config.apply_env_overrides();
        Ok(config)
    }

    /// Load configuration from the config file
    fn load_from_file() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Err(ConfigError::NotFound);
        }
        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get the config file path
    fn config_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir()
            .ok_or(ConfigError::NotFound)?;
        Ok(config_dir.join("agent-rs").join("config.toml"))
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        if let Ok(api_key) = std::env::var("AGENT_RS_API_KEY") {
            self.api_key = api_key;
        }
        if let Ok(model) = std::env::var("AGENT_RS_MODEL") {
            self.model = model;
        }
        if let Ok(max_history) = std::env::var("AGENT_RS_MAX_HISTORY") {
            if let Ok(val) = max_history.parse() {
                self.max_history_messages = val;
            }
        }
        if let Ok(max_bytes) = std::env::var("AGENT_RS_MAX_OUTPUT_BYTES") {
            if let Ok(val) = max_bytes.parse() {
                self.max_output_bytes = val;
            }
        }
        if let Ok(max_lines) = std::env::var("AGENT_RS_MAX_OUTPUT_LINES") {
            if let Ok(val) = max_lines.parse() {
                self.max_output_lines = val;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::TempDir;
    use std::sync::Mutex;

    // Global mutex to serialize tests that modify environment variables
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // **Feature: agent-rs, Property 22: Config Loading Precedence**
    // *For any* configuration value, environment variables SHALL take precedence
    // over config file values, and config file values SHALL take precedence over defaults.
    // **Validates: Requirements 2.1**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_env_overrides_config_file(
            file_api_key in "[a-zA-Z0-9]{10,20}",
            file_model in "[a-zA-Z0-9_-]{5,15}",
            file_max_history in 1usize..100,
            env_api_key in "[a-zA-Z0-9]{10,20}",
            env_model in "[a-zA-Z0-9_-]{5,15}",
            env_max_history in 1usize..100,
        ) {
            // Serialize access to environment variables
            let _guard = ENV_MUTEX.lock().unwrap();

            // Create a temp config file
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.toml");
            
            let config_content = format!(
                r#"
api_key = "{}"
model = "{}"
max_history_messages = {}
"#,
                file_api_key, file_model, file_max_history
            );
            std::fs::write(&config_path, config_content).unwrap();

            // Set environment variables (unsafe in Rust 2024)
            unsafe {
                std::env::set_var("AGENT_RS_API_KEY", &env_api_key);
                std::env::set_var("AGENT_RS_MODEL", &env_model);
                std::env::set_var("AGENT_RS_MAX_HISTORY", &env_max_history.to_string());
            }

            // Load config
            let config = Config::load_with_path(Some(&config_path)).unwrap();

            // Environment variables should take precedence
            prop_assert_eq!(config.api_key, env_api_key);
            prop_assert_eq!(config.model, env_model);
            prop_assert_eq!(config.max_history_messages, env_max_history);

            // Clean up env vars
            unsafe {
                std::env::remove_var("AGENT_RS_API_KEY");
                std::env::remove_var("AGENT_RS_MODEL");
                std::env::remove_var("AGENT_RS_MAX_HISTORY");
            }
        }

        #[test]
        fn prop_config_file_overrides_defaults(
            file_api_key in "[a-zA-Z0-9]{10,20}",
            file_model in "[a-zA-Z0-9_-]{5,15}",
            file_max_history in 1usize..100,
        ) {
            // Serialize access to environment variables
            let _guard = ENV_MUTEX.lock().unwrap();

            // Ensure no env vars are set
            unsafe {
                std::env::remove_var("AGENT_RS_API_KEY");
                std::env::remove_var("AGENT_RS_MODEL");
                std::env::remove_var("AGENT_RS_MAX_HISTORY");
            }

            // Create a temp config file
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.toml");
            
            let config_content = format!(
                r#"
api_key = "{}"
model = "{}"
max_history_messages = {}
"#,
                file_api_key, file_model, file_max_history
            );
            std::fs::write(&config_path, config_content).unwrap();

            // Load config
            let config = Config::load_with_path(Some(&config_path)).unwrap();

            // Config file values should override defaults
            prop_assert_eq!(config.api_key, file_api_key);
            prop_assert_eq!(config.model, file_model);
            prop_assert_eq!(config.max_history_messages, file_max_history);
        }

        #[test]
        fn prop_defaults_used_when_no_file_or_env(
            _dummy in 0..1, // Just to make it a property test
        ) {
            // Serialize access to environment variables
            let _guard = ENV_MUTEX.lock().unwrap();

            // Ensure no env vars are set
            unsafe {
                std::env::remove_var("AGENT_RS_API_KEY");
                std::env::remove_var("AGENT_RS_MODEL");
                std::env::remove_var("AGENT_RS_MAX_HISTORY");
                std::env::remove_var("AGENT_RS_MAX_OUTPUT_BYTES");
                std::env::remove_var("AGENT_RS_MAX_OUTPUT_LINES");
            }

            // Load config with no file
            let config = Config::load_with_path(None).unwrap();
            let defaults = Config::default();

            // Should use defaults
            prop_assert_eq!(config.api_key, defaults.api_key);
            prop_assert_eq!(config.model, defaults.model);
            prop_assert_eq!(config.max_history_messages, defaults.max_history_messages);
            prop_assert_eq!(config.max_output_bytes, defaults.max_output_bytes);
            prop_assert_eq!(config.max_output_lines, defaults.max_output_lines);
        }
    }
}
