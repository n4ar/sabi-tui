# Sabi-TUI

A terminal-based AI agent implementing the ReAct (Reasoning + Acting) pattern for system administration. Describe tasks in natural language, review AI-generated shell commands, and get analysis of results.

## Features

- 🧠 **Gemini AI powered** - Natural language to shell command translation
- 💻 **Terminal access** - Execute commands with safety checks
- 🐍 **Python executor** - Run Python code for calculations (auto-detected)
- 🔒 **Safe mode** - Preview commands without execution
- 💾 **Multi-session** - Save and switch between conversation sessions
- ⚠️ **Dangerous command detection** - Visual warnings for risky commands
- 🚫 **Interactive command blocking** - Prevents hanging on vim, ssh, etc.

## Installation

### Quick Install (Recommended)

Downloads pre-built binary automatically:

```bash
curl -sSL https://raw.githubusercontent.com/n4ar/sabi-tui/main/setup.sh | bash
```

### Manual Download

Download from [Releases](https://github.com/n4ar/sabi-tui/releases):

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `sabi-macos-aarch64` |
| macOS (Intel) | `sabi-macos-x86_64` |
| Linux (x64) | `sabi-linux-x86_64` |
| Linux (ARM64) | `sabi-linux-aarch64` |
| Windows | `sabi-windows-x86_64.exe` |

```bash
# Example for macOS Apple Silicon
curl -L https://github.com/n4ar/sabi-tui/releases/latest/download/sabi-macos-aarch64 -o sabi
chmod +x sabi
mv sabi ~/.local/bin/
```

### Build from Source

```bash
git clone https://github.com/n4ar/sabi-tui.git
cd sabi-tui
cargo build --release
cp target/release/sabi ~/.local/bin/
```

## Requirements

- Gemini API key ([Get one here](https://aistudio.google.com/apikey))

## Configuration

### Option 1: Environment Variable

```bash
export SABI_API_KEY="your-gemini-api-key"
```

### Option 2: Config File

Edit `~/.config/sabi/config.toml`:

```toml
api_key = "your-gemini-api-key"
model = "gemini-2.5-flash"           # optional
max_history_messages = 20            # optional
safe_mode = false                    # optional
dangerous_patterns = ["rm -rf", "mkfs", "dd if="]  # optional
```

## Usage

```bash
# Normal mode
sabi

# Safe mode (preview only, no execution)
sabi --safe

# Help
sabi --help
```

### Basic Workflow

1. **Type your query** in natural language
   ```
   > list all files larger than 100MB
   ```

2. **Review the proposed command**
   ```
   ┌─ Command ─────────────────────────────┐
   │ find ~ -type f -size +100M            │
   └───────────────────────────────────────┘
   ```

3. **Execute or Cancel**
   - `Enter` - Execute the command
   - `Esc` - Cancel and return to input

4. **View AI analysis** of the results

### Slash Commands

| Command | Description |
|---------|-------------|
| `/new` | Start new session |
| `/sessions` | List all sessions |
| `/switch <id>` | Switch to session |
| `/delete <id>` | Delete session |
| `/clear` | Clear chat history |
| `/help` | Show help |
| `/quit` | Exit |

### Keybindings

| State | Key | Action |
|-------|-----|--------|
| Input | `Enter` | Submit query |
| Input | `Esc` | Quit |
| Input | `↑`/`↓` | Scroll history |
| Review | `Enter` | Execute command |
| Review | `Esc` | Cancel |
| Executing | `Esc` | Cancel command |
| Any | `Ctrl+C` | Force quit |

### Status Bar Indicators

| Icon | Meaning |
|------|---------|
| 🐍 | Python available |
| 🔒 SAFE | Safe mode enabled |

## Safety Features

### Dangerous Command Warning

Commands matching dangerous patterns show a red warning:

```
┌─ ⚠ DANGEROUS COMMAND ─────────────────┐
│ rm -rf ./node_modules                  │
└────────────────────────────────────────┘
```

### Interactive Command Blocking

Interactive commands (vim, ssh, htop, etc.) are blocked with suggestions:

```
⚠️ Cannot run interactive command: `vim file.txt`
Use write_file tool instead
```

## Architecture

```
Input → Thinking → ReviewAction → Executing → Finalizing → Input
          ↓              ↓             ↓
        (text)       (cancel)     (cancel)
          ↓              ↓             ↓
        Input ←──────────┴─────────────┘
```

## Available Tools

The AI can use these tools:

| Tool | Description |
|------|-------------|
| `run_cmd` | Execute shell command |
| `run_python` | Execute Python code |
| `read_file` | Read file contents |
| `write_file` | Write to file |
| `search` | Search for files |

## Troubleshooting

### "API key not found"
Set `SABI_API_KEY` environment variable or edit `~/.config/sabi/config.toml`

### "Terminal too small"
Resize terminal to at least 40x10 characters

### Command not executing
Make sure you press `Enter` in Review state, not `Esc`

### Python not detected
Install Python 3: `brew install python3` (macOS) or `apt install python3` (Linux)

## Uninstall

```bash
rm ~/.local/bin/sabi
rm -rf ~/.config/sabi
rm -rf ~/Library/Application\ Support/sabi  # macOS
rm -rf ~/.local/share/sabi                   # Linux
```

## License

MIT
