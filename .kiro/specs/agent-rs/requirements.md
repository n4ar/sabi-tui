# Requirements Document

## Introduction

**agent-rs** is a terminal-based AI agent that acts as a Linux/macOS system expert. Unlike traditional chatbots that only provide text responses, agent-rs proposes executable shell commands, allows users to review and edit them before execution, captures the output, and feeds it back to the AI for analysis. This implements the ReAct (Reasoning + Acting) pattern in a TUI environment.

The tool bridges the gap between natural language queries and system administration tasks, providing a safe, interactive workflow where users maintain full control over command execution.

## Glossary

- **Agent**: The AI-powered system that interprets user queries and proposes shell commands
- **ReAct Pattern**: A reasoning paradigm where the AI thinks, acts (proposes a tool call), observes the result, and iterates
- **TUI**: Terminal User Interface built with ratatui
- **Command Box**: An editable text area where proposed shell commands are displayed for user review
- **Conversation History**: The sequence of messages exchanged between user and AI, including command outputs
- **Tool Call**: A structured JSON response from the AI requesting command execution
- **System Prompt**: Initial instructions that define the AI's behavior as a system expert

## Requirements

### Requirement 1: Natural Language Query Input

**User Story:** As a user, I want to type natural language queries describing system tasks, so that I can get AI-generated shell commands without memorizing syntax.

#### Acceptance Criteria

1. WHEN the application starts THEN the Agent SHALL display an input area where the user can type queries
2. WHEN a user types a query and presses Enter THEN the Agent SHALL send the query to the AI for processing
3. WHEN the input field is empty and the user presses Enter THEN the Agent SHALL ignore the submission and maintain the current state
4. WHEN the user presses Escape in the input state THEN the Agent SHALL exit the application gracefully

### Requirement 2: AI Command Generation

**User Story:** As a user, I want the AI to analyze my query and generate appropriate shell commands, so that I can accomplish system tasks efficiently.

#### Acceptance Criteria

1. WHEN the Agent sends a query to the Gemini API THEN the Agent SHALL include a system prompt defining the AI as a macOS/Linux expert with tool-calling capabilities
2. WHEN the AI determines a shell command is needed THEN the Agent SHALL parse a JSON tool call response in the format `{"tool": "run_cmd", "command": "..."}`
3. WHEN the AI provides a direct text answer without tool calls THEN the Agent SHALL display the response in the chat history
4. WHEN the API request fails THEN the Agent SHALL display an error message and return to the input state
5. WHEN waiting for AI response THEN the Agent SHALL display a visual indicator (spinner) to show processing status

### Requirement 3: Command Review and Editing

**User Story:** As a user, I want to review and optionally edit AI-proposed commands before execution, so that I can ensure safety and correctness.

#### Acceptance Criteria

1. WHEN the AI proposes a command THEN the Agent SHALL display the command in an editable text area with a distinct visual border
2. WHILE in review state THEN the Agent SHALL allow the user to modify the command text using standard text editing operations
3. WHEN the user presses Enter in review state THEN the Agent SHALL execute the displayed command
4. WHEN the user presses Escape in review state THEN the Agent SHALL cancel the command and return to input state
5. WHEN displaying the command for review THEN the Agent SHALL use a green border to indicate the editable command box

### Requirement 4: Command Execution

**User Story:** As a user, I want the application to execute approved commands and capture their output, so that I can see results without switching terminals.

#### Acceptance Criteria

1. WHEN the user confirms command execution THEN the Agent SHALL execute the command using the system shell
2. WHEN a command executes THEN the Agent SHALL capture both stdout and stderr streams
3. WHEN command execution completes THEN the Agent SHALL display the output in the execution pane
4. WHEN a command produces no output THEN the Agent SHALL display a message indicating successful completion with no output
5. IF command execution fails with a non-zero exit code THEN the Agent SHALL capture and display the error output

### Requirement 5: AI Output Analysis

**User Story:** As a user, I want the AI to analyze command output and provide a summary, so that I can understand the results without parsing raw output.

#### Acceptance Criteria

1. WHEN command execution completes THEN the Agent SHALL send the command and its output back to the AI
2. WHEN the AI receives execution results THEN the Agent SHALL maintain conversation history for context
3. WHEN the AI analyzes the output THEN the Agent SHALL display the summary in the chat history pane
4. WHEN analysis completes THEN the Agent SHALL return to the input state for new queries

### Requirement 6: Conversation History Management

**User Story:** As a user, I want to see the conversation history including my queries, AI responses, and command outputs, so that I can track the interaction flow.

#### Acceptance Criteria

1. WHEN a new message is added to the conversation THEN the Agent SHALL append it to the scrollable chat history pane
2. WHEN the chat history exceeds the visible area THEN the Agent SHALL enable scrolling to view previous messages
3. WHEN displaying messages THEN the Agent SHALL visually distinguish between user queries, AI responses, and command outputs
4. WHEN the conversation history is serialized for API calls THEN the Agent SHALL format messages according to the Gemini API conversation structure

### Requirement 7: State Machine Architecture

**User Story:** As a developer, I want a clear state machine governing application behavior, so that transitions between modes are predictable and maintainable.

#### Acceptance Criteria

1. WHEN the application runs THEN the Agent SHALL maintain one of the following states: Input, Thinking, ReviewAction, Executing, Finalizing, Done
2. WHEN transitioning between states THEN the Agent SHALL update the UI to reflect the current state
3. WHEN in Thinking or Finalizing states THEN the Agent SHALL block user input until the operation completes
4. WHEN an error occurs in any state THEN the Agent SHALL transition to an appropriate recovery state with error feedback

### Requirement 8: TUI Layout and Rendering

**User Story:** As a user, I want a clear, organized terminal interface, so that I can easily understand the current state and interact with the application.

#### Acceptance Criteria

1. WHEN rendering the UI THEN the Agent SHALL display a top pane for scrollable chat history
2. WHEN in ReviewAction state THEN the Agent SHALL display the editable command box in the middle pane
3. WHEN in Executing state THEN the Agent SHALL display live terminal output in the middle pane
4. WHEN rendering the UI THEN the Agent SHALL display a status bar at the bottom showing current state and available keybindings
5. WHEN the terminal is resized THEN the Agent SHALL adapt the layout proportionally

### Requirement 9: Async Event Handling

**User Story:** As a developer, I want non-blocking event handling, so that the UI remains responsive during API calls and command execution.

#### Acceptance Criteria

1. WHEN handling keyboard events THEN the Agent SHALL use async channels to decouple input from processing
2. WHEN making API calls THEN the Agent SHALL execute them asynchronously without blocking the render loop
3. WHEN executing commands THEN the Agent SHALL capture output asynchronously to allow UI updates
4. WHEN multiple events occur THEN the Agent SHALL process them using tokio::select! for concurrent handling
