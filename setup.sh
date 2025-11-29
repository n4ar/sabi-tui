#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Config
REPO="n4ar/sabi-tui"  # Change to your GitHub repo
INSTALL_DIR="$HOME/.local/bin"

echo -e "${GREEN}╔════════════════════════════════════╗${NC}"
echo -e "${GREEN}║       Sabi-TUI Installer           ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════╝${NC}"
echo

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)  OS_NAME="linux" ;;
    darwin) OS_NAME="macos" ;;
    *)      echo -e "${RED}Unsupported OS: $OS${NC}"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)  ARCH_NAME="x86_64" ;;
    arm64|aarch64) ARCH_NAME="aarch64" ;;
    *)             echo -e "${RED}Unsupported architecture: $ARCH${NC}"; exit 1 ;;
esac

BINARY_NAME="sabi-${OS_NAME}-${ARCH_NAME}"
echo -e "Detected: ${GREEN}${OS_NAME} ${ARCH_NAME}${NC}"

# Get latest release URL
echo -e "${GREEN}Fetching latest release...${NC}"
LATEST_URL=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep "browser_download_url.*${BINARY_NAME}" | cut -d '"' -f 4)

if [ -z "$LATEST_URL" ]; then
    echo -e "${YELLOW}No pre-built binary found. Building from source...${NC}"
    
    # Fallback to building from source
    if ! command -v cargo &> /dev/null; then
        echo -e "${YELLOW}Installing Rust...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    cargo build --release
    mkdir -p "$INSTALL_DIR"
    cp target/release/sabi "$INSTALL_DIR/"
else
    # Download pre-built binary
    echo -e "${GREEN}Downloading ${BINARY_NAME}...${NC}"
    mkdir -p "$INSTALL_DIR"
    curl -sL "$LATEST_URL" -o "$INSTALL_DIR/sabi"
    chmod +x "$INSTALL_DIR/sabi"
fi

# Add to PATH if needed
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}Adding $INSTALL_DIR to PATH...${NC}"
    
    SHELL_NAME=$(basename "$SHELL")
    
    case "$SHELL_NAME" in
        zsh)
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.zshrc"
            echo -e "${YELLOW}Added to ~/.zshrc${NC}"
            ;;
        bash)
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
            echo -e "${YELLOW}Added to ~/.bashrc${NC}"
            ;;
        fish)
            mkdir -p "$HOME/.config/fish"
            echo 'set -gx PATH $HOME/.local/bin $PATH' >> "$HOME/.config/fish/config.fish"
            echo -e "${YELLOW}Added to ~/.config/fish/config.fish${NC}"
            ;;
    esac
fi

# Setup config
CONFIG_DIR="$HOME/.config/sabi"
mkdir -p "$CONFIG_DIR"

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cat > "$CONFIG_DIR/config.toml" << 'EOF'
# Sabi Configuration
# Get your API key at: https://aistudio.google.com/apikey

# api_key = "your-gemini-api-key"
model = "gemini-2.5-flash"
max_history_messages = 20
safe_mode = false
EOF
fi

echo
echo -e "${GREEN}✓ Installation complete!${NC}"
echo
echo -e "Next steps:"
echo -e "  1. Get API key: ${YELLOW}https://aistudio.google.com/apikey${NC}"
echo -e "  2. Set key: ${YELLOW}export SABI_API_KEY=\"your-key\"${NC}"
echo -e "  3. Run: ${GREEN}sabi${NC} (restart terminal or source your rc file)"
echo
