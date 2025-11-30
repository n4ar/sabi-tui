//! Sabi-TUI: A terminal-based AI agent implementing the ReAct pattern

mod ai_client;
mod app;
mod config;
mod event;
mod executor;
mod gemini;
mod message;
mod onboarding;
mod openai;
mod state;
mod tool_call;
mod ui;

use std::io::{self, stdout};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use ai_client::AIClient;
use app::{App, InputResult};
use config::Config;
use event::{Event, EventHandler};
use executor::{CommandExecutor, DangerousCommandDetector, InteractiveCommandDetector};
use gemini::SYSTEM_PROMPT;
use message::Message;
use state::StateEvent;
use tool_call::ParsedResponse;

/// Tick rate for UI updates (100ms = 10 FPS)
const TICK_RATE: Duration = Duration::from_millis(100);

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!("sabi - AI-powered terminal assistant\n");
    println!("Usage: sabi [OPTIONS]\n");
    println!("Options:");
    println!("  --safe           Safe mode: show commands but don't execute");
    println!("  --version, -v    Show version");
    println!("  --help, -h       Show this help message");
}

fn print_version() {
    println!("sabi {}", VERSION);
}

/// Get system context for AI
fn get_system_context() -> String {
    let time = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string();
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".into());
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".into());
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".into());

    let (os_name, os_version) = get_os_info();

    format!(
        "SYSTEM CONTEXT:\n\
         - Current time: {}\n\
         - User: {}\n\
         - Shell: {}\n\
         - Working directory: {}\n\
         - OS: {} {}",
        time, user, shell, cwd, os_name, os_version
    )
}

fn get_os_info() -> (String, String) {
    #[cfg(target_os = "macos")]
    {
        let version = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".into());
        ("macOS".into(), version)
    }
    #[cfg(target_os = "linux")]
    {
        let version = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("PRETTY_NAME=")).map(|l| {
                    l.trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_string()
                })
            })
            .unwrap_or_else(|| "Linux".into());
        ("Linux".into(), version)
    }
    #[cfg(target_os = "windows")]
    {
        ("Windows".into(), "".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        ("Unknown".into(), "".into())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--version" || a == "-v") {
        print_version();
        return Ok(());
    }

    let mut config = Config::load().context("Failed to load configuration")?;

    // CLI flag overrides config
    if args.iter().any(|a| a == "--safe") {
        config.safe_mode = true;
    }

    // Run onboarding if no API key configured
    if !config.has_api_key() {
        config = onboarding::run_onboarding().context("Onboarding failed")?;
    }

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    let mut app = App::new(config.clone());
    let mut events = EventHandler::new(TICK_RATE);

    // Gather system context
    let system_context = get_system_context();

    // Build system prompt (include Python tool if available)
    let system_prompt = if app.python_available {
        format!(
            "{}\n\n5. Run Python code:\n   {{\"tool\": \"run_python\", \"code\": \"<python code>\"}}\n\nEXAMPLE:\n- \"calculate 2^100\" → {{\"tool\": \"run_python\", \"code\": \"print(2**100)\"}}\n\n{}",
            SYSTEM_PROMPT, system_context
        )
    } else {
        format!("{}\n\n{}", SYSTEM_PROMPT, system_context)
    };
    app.add_message(Message::system(&system_prompt));

    // Auto-load previous session
    app.auto_load();

    let ai_client = AIClient::new(&config).ok();
    let detector = DangerousCommandDetector::new(&config.dangerous_patterns);
    let interactive_detector = InteractiveCommandDetector::new();

    let result = run_loop(
        &mut terminal,
        &mut app,
        &mut events,
        ai_client,
        detector,
        interactive_detector,
    )
    .await;

    // Auto-save session before exit
    app.auto_save();

    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App<'_>,
    events: &mut EventHandler,
    mut ai_client: Option<AIClient>,
    detector: DangerousCommandDetector,
    interactive_detector: InteractiveCommandDetector,
) -> Result<()> {
    let tx = events.sender();

    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    let result = app.handle_key_event(key);

                    // Handle command cancellation
                    if result == InputResult::CancelCommand {
                        app.add_message(Message::system("⚠️ Command cancelled"));
                        app.transition(StateEvent::AnalysisComplete);
                        continue;
                    }

                    // Handle /model command
                    if let InputResult::FetchModels(model_arg) = result.clone() {
                        if let Some(ref client) = ai_client {
                            let client_clone = client.clone();
                            let tx_clone = tx.clone();
                            tokio::spawn(async move {
                                let models = client_clone.list_models().await;
                                let _ = tx_clone.send(Event::ModelsResponse(models, model_arg));
                            });
                        } else {
                            app.add_message(Message::system("API key not configured"));
                        }
                        continue;
                    }

                    // 12.1: Input → Thinking transition
                    if result == InputResult::SubmitQuery {
                        if let Some(ref client) = ai_client {
                            let messages = app.messages.clone();
                            let client_clone = client.clone();
                            let tx_clone = tx.clone();
                            tokio::spawn(async move {
                                let response = client_clone.chat(&messages).await;
                                let _ = tx_clone.send(Event::ApiResponse(response));
                            });
                        } else {
                            app.set_error("API key not configured");
                            app.transition(StateEvent::ApiError);
                        }
                    }

                    // 12.4: ReviewAction → Executing transition
                    if result == InputResult::ExecuteCommand {
                        if let Some(ref tool) = app.current_tool {
                            // Safe mode: don't execute, just show what would run
                            if app.config.safe_mode {
                                let desc = match tool.tool.as_str() {
                                    "run_cmd" => format!("Would run: {}", tool.command),
                                    "run_python" => format!("Would run Python:\n{}", tool.code),
                                    "read_file" => format!("Would read: {}", tool.path),
                                    "write_file" => format!(
                                        "Would write {} bytes to: {}",
                                        tool.content.len(),
                                        tool.path
                                    ),
                                    "search" => format!(
                                        "Would search '{}' in {}",
                                        tool.pattern, tool.directory
                                    ),
                                    _ => format!("Would execute: {:?}", tool),
                                };
                                app.add_message(Message::system(&format!(
                                    "🔒 [SAFE MODE] {}",
                                    desc
                                )));
                                app.transition(StateEvent::AnalysisComplete);
                            } else {
                                let tool = tool.clone();
                                let exec = CommandExecutor::new(&app.config);
                                let tx_clone = tx.clone();
                                let handle = tokio::spawn(async move {
                                    let result = exec.execute_tool_async(&tool).await;
                                    let _ = tx_clone.send(Event::CommandComplete(result));
                                });
                                app.running_task = Some(handle);
                            }
                        }
                    }
                }
                Event::Tick => {
                    app.tick_spinner();
                }
                Event::Resize(_, _) => {}

                // 12.2: Thinking → ReviewAction/Input transition
                Event::ApiResponse(response) => {
                    match response {
                        Ok(text) => {
                            app.add_message(Message::model(&text));

                            match ParsedResponse::parse(&text) {
                                ParsedResponse::ToolCall(tc) => {
                                    // Format display text based on tool type
                                    let display = match tc.tool.as_str() {
                                        "run_cmd" => tc.command.clone(),
                                        "run_python" => format!("python:\n{}", tc.code),
                                        "read_file" => format!("read_file: {}", tc.path),
                                        "write_file" => format!(
                                            "write_file: {} ({} bytes)",
                                            tc.path,
                                            tc.content.len()
                                        ),
                                        "search" => format!(
                                            "search: {} in {}",
                                            tc.pattern,
                                            if tc.directory.is_empty() {
                                                "."
                                            } else {
                                                &tc.directory
                                            }
                                        ),
                                        _ => format!("{:?}", tc),
                                    };

                                    // Check for interactive commands
                                    if tc.is_run_cmd()
                                        && interactive_detector.is_interactive(&tc.command)
                                    {
                                        let suggestion =
                                            interactive_detector.suggestion(&tc.command).unwrap_or(
                                                "This command requires an interactive terminal",
                                            );
                                        app.add_message(Message::model(&format!(
                                            "⚠️ Cannot run interactive command: `{}`\n{}",
                                            tc.command, suggestion
                                        )));
                                        app.transition(StateEvent::TextResponseReceived);
                                        continue;
                                    }

                                    // Check Python availability
                                    if tc.tool == "run_python" && !app.python_available {
                                        app.add_message(Message::model(
                                            "⚠️ Python is not available on this system.\nPlease install Python 3 to use this feature."
                                        ));
                                        app.transition(StateEvent::TextResponseReceived);
                                        continue;
                                    }

                                    app.set_action_text(&display);
                                    app.current_tool = Some(tc.clone());

                                    // Check for dangerous operations
                                    app.dangerous_command_detected = tc.is_destructive()
                                        || (tc.is_run_cmd() && detector.is_dangerous(&tc.command));

                                    // Block unknown tools entirely
                                    if !tc.is_allowed_tool() {
                                        app.add_message(Message::system(&format!(
                                            "⛔ Blocked unknown tool: '{}'\nAllowed: run_cmd, read_file, write_file, search, run_python",
                                            tc.tool
                                        )));
                                        app.transition(StateEvent::TextResponseReceived);
                                        continue;
                                    }

                                    app.transition(StateEvent::ToolCallReceived);
                                }
                                _ => {
                                    app.transition(StateEvent::TextResponseReceived);
                                }
                            }
                        }
                        Err(e) => {
                            app.set_error(e.to_string());
                            app.transition(StateEvent::ApiError);
                        }
                    }
                }

                // 12.5: Executing → Finalizing → Input loop
                Event::CommandComplete(result) => {
                    app.running_task = None;
                    app.execution_output = if result.success {
                        result.stdout.clone()
                    } else {
                        format!("{}\n{}", result.stdout, result.stderr)
                    };

                    let tool_desc = app
                        .current_tool
                        .as_ref()
                        .map(|t| {
                            format!(
                                "{}: {}",
                                t.tool,
                                if t.tool == "run_cmd" {
                                    &t.command
                                } else {
                                    &t.path
                                }
                            )
                        })
                        .unwrap_or_default();

                    let feedback = format!(
                        "Tool: {}\nExit code: {}\nOutput:\n{}",
                        tool_desc, result.exit_code, &app.execution_output
                    );
                    app.add_message(Message::user(&feedback));
                    app.transition(StateEvent::CommandComplete);

                    // Send to AI for analysis
                    if let Some(ref client) = ai_client {
                        let messages = app.messages.clone();
                        let client_clone = client.clone();
                        let tx_clone = tx.clone();
                        tokio::spawn(async move {
                            let response = client_clone.chat(&messages).await;
                            let _ = tx_clone.send(Event::ApiResponse(response));
                        });
                    } else {
                        app.transition(StateEvent::AnalysisComplete);
                    }
                }

                Event::CommandCancelled => {
                    // Task was cancelled, already handled in key event
                }

                Event::ModelsResponse(result, model_arg) => {
                    match result {
                        Ok(models) => {
                            if let Some(model_name) = model_arg {
                                // Switch to specified model
                                if let Some(matched) =
                                    models.iter().find(|m| m.contains(&model_name))
                                {
                                    if let Some(ref mut client) = ai_client {
                                        client.set_model(matched.clone());
                                        app.add_message(Message::system(&format!(
                                            "✓ Switched to: {}",
                                            matched
                                        )));
                                    }
                                } else {
                                    app.add_message(Message::system(&format!(
                                        "✗ Model '{}' not found",
                                        model_name
                                    )));
                                }
                            } else {
                                // List all models
                                let current =
                                    ai_client.as_ref().map(|c| c.model()).unwrap_or("unknown");
                                let list = models
                                    .iter()
                                    .map(|m| {
                                        if m == current {
                                            format!("→ {}", m)
                                        } else {
                                            format!("  {}", m)
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                app.add_message(Message::system(&format!(
                                    "Available models:\n{}\n\nUse /model <name> to switch",
                                    list
                                )));
                            }
                        }
                        Err(e) => {
                            app.add_message(Message::system(&format!(
                                "✗ Failed to fetch models: {}",
                                e
                            )));
                        }
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
