# Changelog

## [0.1.3] - 2025-11-30

### Added
- 🐍 **Python executor** - Run Python code with `run_python` tool (auto-detected at startup)
- 🔒 **Safe mode** - Preview commands without execution (`sabi --safe`)
- 💾 **Multi-session support** - `/new`, `/sessions`, `/switch <id>`, `/delete <id>`
- 🚫 **Interactive command blocking** - Detects and blocks vim, ssh, htop, etc. with suggestions
- ⏹️ **Cancel running commands** - Press `Esc` during execution to abort
- 🖥️ **System context** - AI knows current time, user, shell, OS, and working directory
- 📦 **Pre-built binaries** - Download from GitHub Releases (macOS, Linux)
- 🐟 **Fish shell support** - setup.sh now supports fish

### Changed
- Renamed project from `agent-rs` to `Sabi-TUI`
- Binary name changed to `sabi`
- Config path: `~/.config/sabi/config.toml`
- Environment variable: `SABI_API_KEY`
- Session storage: `~/Library/Application Support/sabi/sessions/` (macOS)
- Middle pane now auto-sizes based on content
- Switched to `rustls` for cross-compilation support

### Fixed
- Commands no longer hang on interactive programs
- Session auto-save on exit

## [0.1.0] - 2025-11-29

### Added
- Initial release
- Gemini AI integration
- ReAct pattern implementation
- Shell command execution
- File read/write tools
- Dangerous command detection
- TUI interface with ratatui
