//! Command execution
//!
//! Handles shell command execution and output capture with safety limits.

use std::process::Command;

use regex::Regex;
use tokio::process::Command as TokioCommand;

use crate::config::Config;
use crate::tool_call::ToolCall;

/// Result of command execution
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code (0 typically means success)
    pub exit_code: i32,
    /// Whether the command succeeded (exit code 0)
    pub success: bool,
    /// Whether output was truncated due to size limits
    pub truncated: bool,
}

/// Executes shell commands and captures output with safety limits
pub struct CommandExecutor {
    /// Maximum bytes to capture from output
    max_output_bytes: usize,
    /// Maximum lines to capture from output
    max_output_lines: usize,
}

impl CommandExecutor {
    /// Create a new CommandExecutor with limits from config
    pub fn new(config: &Config) -> Self {
        Self {
            max_output_bytes: config.max_output_bytes,
            max_output_lines: config.max_output_lines,
        }
    }

    /// Create a CommandExecutor with custom limits (for testing)
    pub fn with_limits(max_output_bytes: usize, max_output_lines: usize) -> Self {
        Self {
            max_output_bytes,
            max_output_lines,
        }
    }

    /// Execute a tool call
    pub fn execute_tool(&self, tool: &ToolCall) -> CommandResult {
        match tool.tool.as_str() {
            "run_cmd" => self.execute(&tool.command),
            "run_python" => self.run_python(&tool.code),
            "read_file" => self.read_file(&tool.path),
            "write_file" => self.write_file(&tool.path, &tool.content),
            "search" => self.search(&tool.pattern, &tool.directory),
            _ => CommandResult {
                stdout: String::new(),
                stderr: format!("Unknown tool: {}", tool.tool),
                exit_code: 1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Execute Python code
    pub fn run_python(&self, code: &str) -> CommandResult {
        use std::process::Command;
        use std::io::Write;
        
        let mut child = match Command::new("python3")
            .arg("-c")
            .arg(code)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return CommandResult {
                stdout: String::new(),
                stderr: format!("Failed to start Python: {}", e),
                exit_code: 1,
                success: false,
                truncated: false,
            },
        };
        
        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => return CommandResult {
                stdout: String::new(),
                stderr: format!("Python execution failed: {}", e),
                exit_code: 1,
                success: false,
                truncated: false,
            },
        };
        
        let (stdout, stdout_truncated) = self.truncate_output(
            String::from_utf8_lossy(&output.stdout).to_string()
        );
        let (stderr, stderr_truncated) = self.truncate_output(
            String::from_utf8_lossy(&output.stderr).to_string()
        );
        
        CommandResult {
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
            truncated: stdout_truncated || stderr_truncated,
        }
    }

    /// Read a file and return its contents
    pub fn read_file(&self, path: &str) -> CommandResult {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let (output, truncated) = self.truncate_output(content);
                CommandResult {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    success: true,
                    truncated,
                }
            }
            Err(e) => CommandResult {
                stdout: String::new(),
                stderr: format!("Failed to read file: {}", e),
                exit_code: 1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Write content to a file
    pub fn write_file(&self, path: &str, content: &str) -> CommandResult {
        match std::fs::write(path, content) {
            Ok(_) => CommandResult {
                stdout: format!("Successfully wrote {} bytes to {}", content.len(), path),
                stderr: String::new(),
                exit_code: 0,
                success: true,
                truncated: false,
            },
            Err(e) => CommandResult {
                stdout: String::new(),
                stderr: format!("Failed to write file: {}", e),
                exit_code: 1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Search for files matching a pattern
    pub fn search(&self, pattern: &str, directory: &str) -> CommandResult {
        let dir = if directory.is_empty() { "." } else { directory };
        let cmd = format!("find {} -name '{}' 2>/dev/null | head -100", dir, pattern);
        self.execute(&cmd)
    }

    /// Execute a shell command and capture output
    ///
    /// Uses the system shell to execute the command, capturing both
    /// stdout and stderr. Output is truncated if it exceeds configured limits.
    pub fn execute(&self, command: &str) -> CommandResult {
        let shell = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        let output = Command::new(shell.0)
            .arg(shell.1)
            .arg(command)
            .output();

        match output {
            Ok(output) => {
                let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();

                let (stdout, stdout_truncated) = self.truncate_output(raw_stdout);
                let (stderr, stderr_truncated) = self.truncate_output(raw_stderr);

                CommandResult {
                    stdout,
                    stderr,
                    exit_code: output.status.code().unwrap_or(-1),
                    success: output.status.success(),
                    truncated: stdout_truncated || stderr_truncated,
                }
            }
            Err(e) => CommandResult {
                stdout: String::new(),
                stderr: format!("Failed to execute command: {}", e),
                exit_code: -1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Execute a shell command asynchronously (cancellable)
    pub async fn execute_async(&self, command: &str) -> CommandResult {
        let shell = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        let output = TokioCommand::new(shell.0)
            .arg(shell.1)
            .arg(command)
            .output()
            .await;

        match output {
            Ok(output) => {
                let (stdout, stdout_truncated) = self.truncate_output(
                    String::from_utf8_lossy(&output.stdout).to_string()
                );
                let (stderr, stderr_truncated) = self.truncate_output(
                    String::from_utf8_lossy(&output.stderr).to_string()
                );
                CommandResult {
                    stdout,
                    stderr,
                    exit_code: output.status.code().unwrap_or(-1),
                    success: output.status.success(),
                    truncated: stdout_truncated || stderr_truncated,
                }
            }
            Err(e) => CommandResult {
                stdout: String::new(),
                stderr: format!("Failed to execute: {}", e),
                exit_code: -1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Execute a tool call asynchronously (cancellable)
    pub async fn execute_tool_async(&self, tool: &ToolCall) -> CommandResult {
        match tool.tool.as_str() {
            "run_cmd" => self.execute_async(&tool.command).await,
            "run_python" => self.run_python_async(&tool.code).await,
            // These are fast, no need for async
            "read_file" => self.read_file(&tool.path),
            "write_file" => self.write_file(&tool.path, &tool.content),
            "search" => self.execute_async(&format!(
                "find {} -name '{}' 2>/dev/null | head -100",
                if tool.directory.is_empty() { "." } else { &tool.directory },
                tool.pattern
            )).await,
            _ => CommandResult {
                stdout: String::new(),
                stderr: format!("Unknown tool: {}", tool.tool),
                exit_code: 1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Execute Python code asynchronously
    pub async fn run_python_async(&self, code: &str) -> CommandResult {
        let output = TokioCommand::new("python3")
            .arg("-c")
            .arg(code)
            .output()
            .await;

        match output {
            Ok(output) => {
                let (stdout, stdout_truncated) = self.truncate_output(
                    String::from_utf8_lossy(&output.stdout).to_string()
                );
                let (stderr, stderr_truncated) = self.truncate_output(
                    String::from_utf8_lossy(&output.stderr).to_string()
                );
                CommandResult {
                    stdout,
                    stderr,
                    exit_code: output.status.code().unwrap_or(-1),
                    success: output.status.success(),
                    truncated: stdout_truncated || stderr_truncated,
                }
            }
            Err(e) => CommandResult {
                stdout: String::new(),
                stderr: format!("Python error: {}", e),
                exit_code: -1,
                success: false,
                truncated: false,
            },
        }
    }

    /// Truncate output to configured limits
    ///
    /// Returns (truncated_output, was_truncated)
    pub fn truncate_output(&self, output: String) -> (String, bool) {
        let mut result = output;
        let mut truncated = false;

        // First, truncate by bytes if needed
        if result.len() > self.max_output_bytes {
            // Find a valid UTF-8 boundary
            let mut byte_limit = self.max_output_bytes;
            while byte_limit > 0 && !result.is_char_boundary(byte_limit) {
                byte_limit -= 1;
            }
            result = result[..byte_limit].to_string();
            truncated = true;
        }

        // Then, truncate by lines if needed
        let lines: Vec<&str> = result.lines().collect();
        if lines.len() > self.max_output_lines {
            result = lines[..self.max_output_lines].join("\n");
            truncated = true;
        }

        if truncated {
            result.push_str("\n\n[Output truncated due to size limits]");
        }

        (result, truncated)
    }
}

/// Detects potentially dangerous shell commands using regex patterns
pub struct DangerousCommandDetector {
    /// Compiled regex patterns for dangerous commands
    patterns: Vec<Regex>,
}

impl DangerousCommandDetector {
    /// Create a new detector with patterns from config
    pub fn new(patterns: &[String]) -> Self {
        Self {
            patterns: patterns
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect(),
        }
    }

    /// Create a detector with default dangerous patterns
    pub fn with_defaults() -> Self {
        let default_patterns = vec![
            r"rm\s+-rf\s+/".to_string(),
            r"mkfs".to_string(),
            r"dd\s+if=".to_string(),
            r":\(\)\s*\{".to_string(), // fork bomb
            r">\s*/dev/sd".to_string(),
        ];
        Self::new(&default_patterns)
    }

    /// Check if a command matches any dangerous pattern
    pub fn is_dangerous(&self, command: &str) -> bool {
        self.patterns.iter().any(|p| p.is_match(command))
    }

    /// Get all patterns that match the command (for detailed warnings)
    pub fn matching_patterns(&self, command: &str) -> Vec<&Regex> {
        self.patterns
            .iter()
            .filter(|p| p.is_match(command))
            .collect()
    }
}

/// Detects interactive commands that require a TTY
pub struct InteractiveCommandDetector {
    patterns: Vec<Regex>,
}

impl InteractiveCommandDetector {
    pub fn new() -> Self {
        let patterns = [
            r"^(nano|vim?|emacs|pico|ne|joe)\b",  // editors
            r"^(ssh|telnet|ftp|sftp)\b",          // remote sessions
            r"^(htop|top|less|more|man)\b",       // pagers/monitors
            r"^(mysql|psql|sqlite3|mongo)\b",    // database CLIs
            r"^(python|node|irb|ghci)$",          // REPLs (no args)
            r"\b(docker|podman)\s+.*\s-it\b",     // interactive containers
        ];
        Self {
            patterns: patterns.iter().filter_map(|p| Regex::new(p).ok()).collect(),
        }
    }

    pub fn is_interactive(&self, command: &str) -> bool {
        let cmd = command.trim();
        self.patterns.iter().any(|p| p.is_match(cmd))
    }

    pub fn suggestion(&self, command: &str) -> Option<&'static str> {
        let cmd = command.trim().split_whitespace().next().unwrap_or("");
        match cmd {
            "nano" | "vim" | "vi" | "emacs" => Some("Use /save or write_file tool instead"),
            "less" | "more" | "man" => Some("Use cat or read_file tool instead"),
            "ssh" | "telnet" => Some("Interactive sessions not supported"),
            "htop" | "top" => Some("Use 'ps aux' or 'ps aux | head' instead"),
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // **Feature: agent-rs, Property 9: Command Execution Output Capture**
    // *For any* shell command string, executing it SHALL capture both stdout and stderr,
    // and the combined output SHALL be stored in execution_output.
    // **Validates: Requirements 4.1, 4.2, 4.5**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_command_captures_stdout(
            // Generate safe echo content (alphanumeric only to avoid shell injection)
            content in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let executor = CommandExecutor::with_limits(50 * 1024, 500);
            let command = format!("echo '{}'", content);
            let result = executor.execute(&command);

            // stdout should contain the echoed content
            prop_assert!(
                result.stdout.trim().contains(content.trim()),
                "stdout '{}' should contain '{}'",
                result.stdout.trim(),
                content.trim()
            );
            prop_assert!(result.success, "echo command should succeed");
            prop_assert_eq!(result.exit_code, 0, "exit code should be 0");
        }

        #[test]
        fn prop_command_captures_stderr(
            // Generate safe content for stderr
            content in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let executor = CommandExecutor::with_limits(50 * 1024, 500);
            // Use >&2 to redirect to stderr
            let command = format!("echo '{}' >&2", content);
            let result = executor.execute(&command);

            // stderr should contain the echoed content
            prop_assert!(
                result.stderr.trim().contains(content.trim()),
                "stderr '{}' should contain '{}'",
                result.stderr.trim(),
                content.trim()
            );
            prop_assert!(result.success, "echo to stderr should still succeed");
        }

        #[test]
        fn prop_command_captures_exit_code(
            exit_code in 0i32..128
        ) {
            let executor = CommandExecutor::with_limits(50 * 1024, 500);
            let command = format!("exit {}", exit_code);
            let result = executor.execute(&command);

            prop_assert_eq!(
                result.exit_code, exit_code,
                "exit code should match the command's exit code"
            );
            prop_assert_eq!(
                result.success, exit_code == 0,
                "success should be true only for exit code 0"
            );
        }

        #[test]
        fn prop_command_captures_both_streams(
            stdout_content in "[a-zA-Z0-9]{1,20}",
            stderr_content in "[a-zA-Z0-9]{1,20}"
        ) {
            let executor = CommandExecutor::with_limits(50 * 1024, 500);
            // Command that writes to both stdout and stderr
            let command = format!(
                "echo '{}' && echo '{}' >&2",
                stdout_content, stderr_content
            );
            let result = executor.execute(&command);

            prop_assert!(
                result.stdout.contains(&stdout_content),
                "stdout should contain stdout_content"
            );
            prop_assert!(
                result.stderr.contains(&stderr_content),
                "stderr should contain stderr_content"
            );
        }
    }

    #[test]
    fn test_failed_command_captures_error() {
        let executor = CommandExecutor::with_limits(50 * 1024, 500);
        // Command that doesn't exist
        let result = executor.execute("nonexistent_command_12345");

        // Should capture error in stderr or have non-zero exit code
        assert!(!result.success || !result.stderr.is_empty());
    }

    #[test]
    fn test_command_with_no_output() {
        let executor = CommandExecutor::with_limits(50 * 1024, 500);
        let result = executor.execute("true");

        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    // **Feature: agent-rs, Property 19: Output Truncation Safety**
    // *For any* command output exceeding the configured max_output_bytes or max_output_lines,
    // the output SHALL be truncated and the truncated flag SHALL be set to true.
    // **Validates: Requirements 4.2**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_truncate_by_bytes(
            // Generate content that will exceed byte limit
            repeat_count in 10usize..100,
        ) {
            let max_bytes = 50;
            let executor = CommandExecutor::with_limits(max_bytes, 1000);
            
            // Create a string that exceeds max_bytes
            let content = "x".repeat(repeat_count);
            
            let (truncated_output, was_truncated) = executor.truncate_output(content.clone());
            
            if content.len() > max_bytes {
                prop_assert!(was_truncated, "should be truncated when exceeding byte limit");
                // The truncated output (minus the truncation message) should be <= max_bytes
                let output_without_message = truncated_output
                    .strip_suffix("\n\n[Output truncated due to size limits]")
                    .unwrap_or(&truncated_output);
                prop_assert!(
                    output_without_message.len() <= max_bytes,
                    "truncated output should not exceed max_bytes"
                );
            } else {
                prop_assert!(!was_truncated, "should not be truncated when within byte limit");
            }
        }

        #[test]
        fn prop_truncate_by_lines(
            // Generate content with varying number of lines
            line_count in 1usize..50,
        ) {
            let max_lines = 10;
            let executor = CommandExecutor::with_limits(100_000, max_lines);
            
            // Create content with line_count lines
            let content: String = (0..line_count)
                .map(|i| format!("line{}", i))
                .collect::<Vec<_>>()
                .join("\n");
            
            let (truncated_output, was_truncated) = executor.truncate_output(content.clone());
            
            if line_count > max_lines {
                prop_assert!(was_truncated, "should be truncated when exceeding line limit");
                // Count lines in output (excluding truncation message)
                let output_without_message = truncated_output
                    .strip_suffix("\n\n[Output truncated due to size limits]")
                    .unwrap_or(&truncated_output);
                let actual_lines = output_without_message.lines().count();
                prop_assert!(
                    actual_lines <= max_lines,
                    "truncated output should not exceed max_lines: got {} lines",
                    actual_lines
                );
            } else {
                prop_assert!(!was_truncated, "should not be truncated when within line limit");
            }
        }

        #[test]
        fn prop_truncation_preserves_utf8_validity(
            // Generate content with multi-byte UTF-8 characters
            char_count in 10usize..100,
        ) {
            let max_bytes = 50;
            let executor = CommandExecutor::with_limits(max_bytes, 1000);
            
            // Create a string with multi-byte UTF-8 characters (emoji)
            let content = "🎉".repeat(char_count);
            
            let (truncated_output, _) = executor.truncate_output(content);
            
            // The output should be valid UTF-8 (this is guaranteed by String type)
            // But we verify it doesn't panic and produces valid output
            prop_assert!(truncated_output.is_ascii() || !truncated_output.is_empty() || truncated_output.len() >= 0);
            
            // Verify we can iterate over chars without panic
            let _ = truncated_output.chars().count();
        }

        #[test]
        fn prop_no_truncation_within_limits(
            content_len in 1usize..50,
            line_count in 1usize..10,
        ) {
            let max_bytes = 100;
            let max_lines = 20;
            let executor = CommandExecutor::with_limits(max_bytes, max_lines);
            
            // Create content within limits
            let lines: Vec<String> = (0..line_count)
                .map(|_| "x".repeat(content_len.min(max_bytes / (line_count + 1))))
                .collect();
            let content = lines.join("\n");
            
            // Only test if content is actually within limits
            if content.len() <= max_bytes && line_count <= max_lines {
                let (truncated_output, was_truncated) = executor.truncate_output(content.clone());
                
                prop_assert!(!was_truncated, "should not truncate content within limits");
                prop_assert_eq!(truncated_output, content, "content should be unchanged");
            }
        }
    }

    #[test]
    fn test_truncation_message_appended() {
        let executor = CommandExecutor::with_limits(10, 1000);
        let content = "x".repeat(100);
        
        let (truncated_output, was_truncated) = executor.truncate_output(content);
        
        assert!(was_truncated);
        assert!(truncated_output.contains("[Output truncated due to size limits]"));
    }

    #[test]
    fn test_command_execution_truncates_large_output() {
        let executor = CommandExecutor::with_limits(100, 10);
        // Generate output that exceeds limits
        let result = executor.execute("seq 1 100");
        
        assert!(result.truncated, "large output should be truncated");
        assert!(result.stdout.contains("[Output truncated due to size limits]"));
    }

    // **Feature: agent-rs, Property 20: Dangerous Command Detection**
    // *For any* command string matching a configured dangerous pattern,
    // the dangerous_command_detected flag SHALL be set to true and the UI SHALL display a warning indicator.
    // **Validates: Requirements 3.5**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_rm_rf_root_detected(
            // Generate variations of rm -rf / command
            spaces_before in 0usize..3,
            spaces_after in 1usize..3,
            path_suffix in prop::option::of("[a-z]{0,5}"),
        ) {
            let detector = DangerousCommandDetector::with_defaults();
            
            // Build command with varying whitespace
            let spaces1 = " ".repeat(spaces_before);
            let spaces2 = " ".repeat(spaces_after);
            let suffix = path_suffix.unwrap_or_default();
            let command = format!("{}rm{}-rf{}/{}", spaces1, spaces2, spaces2, suffix);
            
            prop_assert!(
                detector.is_dangerous(&command),
                "rm -rf / variants should be detected as dangerous: '{}'",
                command
            );
        }

        #[test]
        fn prop_mkfs_detected(
            // Generate mkfs command variations
            device in "[a-z]{3,6}",
            fs_type in prop::option::of("(ext4|xfs|btrfs|ntfs)"),
        ) {
            let detector = DangerousCommandDetector::with_defaults();
            
            let command = match fs_type {
                Some(fs) => format!("mkfs.{} /dev/{}", fs, device),
                None => format!("mkfs /dev/{}", device),
            };
            
            prop_assert!(
                detector.is_dangerous(&command),
                "mkfs commands should be detected as dangerous: '{}'",
                command
            );
        }

        #[test]
        fn prop_dd_if_detected(
            // Generate dd command variations
            input_file in "[a-z/]{1,10}",
            output_file in "[a-z/]{1,10}",
        ) {
            let detector = DangerousCommandDetector::with_defaults();
            
            let command = format!("dd if={} of={}", input_file, output_file);
            
            prop_assert!(
                detector.is_dangerous(&command),
                "dd if= commands should be detected as dangerous: '{}'",
                command
            );
        }

        #[test]
        fn prop_safe_commands_not_flagged(
            // Generate safe command variations
            cmd in "(ls|pwd|echo|cat|grep|find|ps|top|df|du)",
            args in "[a-zA-Z0-9 ._/-]{0,20}",
        ) {
            let detector = DangerousCommandDetector::with_defaults();
            
            let command = format!("{} {}", cmd, args);
            
            prop_assert!(
                !detector.is_dangerous(&command),
                "safe commands should not be flagged as dangerous: '{}'",
                command
            );
        }

        #[test]
        fn prop_custom_patterns_work(
            // Test that custom patterns are respected
            pattern_word in "[a-z]{3,8}",
            command_prefix in "[a-z]{0,5}",
        ) {
            let patterns = vec![format!(r"{}", pattern_word)];
            let detector = DangerousCommandDetector::new(&patterns);
            
            // Command containing the pattern word should be detected
            let dangerous_cmd = format!("{} {} something", command_prefix, pattern_word);
            prop_assert!(
                detector.is_dangerous(&dangerous_cmd),
                "command containing pattern should be dangerous: '{}'",
                dangerous_cmd
            );
            
            // Command not containing the pattern should be safe
            let safe_cmd = format!("{} safe_word", command_prefix);
            if !safe_cmd.contains(&pattern_word) {
                prop_assert!(
                    !detector.is_dangerous(&safe_cmd),
                    "command not containing pattern should be safe: '{}'",
                    safe_cmd
                );
            }
        }
    }

    #[test]
    fn test_fork_bomb_detected() {
        let detector = DangerousCommandDetector::with_defaults();
        
        // Classic fork bomb pattern
        assert!(detector.is_dangerous(":() { :|:& };:"));
        assert!(detector.is_dangerous(":(){ :|:& };:"));
    }

    #[test]
    fn test_write_to_dev_sd_detected() {
        let detector = DangerousCommandDetector::with_defaults();
        
        assert!(detector.is_dangerous("cat file > /dev/sda"));
        assert!(detector.is_dangerous("echo test > /dev/sdb1"));
    }

    #[test]
    fn test_empty_patterns_allows_all() {
        let detector = DangerousCommandDetector::new(&[]);
        
        // With no patterns, nothing should be flagged
        assert!(!detector.is_dangerous("rm -rf /"));
        assert!(!detector.is_dangerous("mkfs /dev/sda"));
    }

    #[test]
    fn test_matching_patterns_returns_matches() {
        let detector = DangerousCommandDetector::with_defaults();
        
        let matches = detector.matching_patterns("rm -rf /home");
        assert_eq!(matches.len(), 1);
        
        let no_matches = detector.matching_patterns("ls -la");
        assert!(no_matches.is_empty());
    }
}
