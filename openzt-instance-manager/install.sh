#!/bin/bash
# OpenZT Instance Manager Installation Script
# This script installs the server and client binaries and sets up a systemd service

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Installation flags (default: install both)
INSTALL_SERVER=true
INSTALL_CLI=true

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --server-only)
            INSTALL_SERVER=true
            INSTALL_CLI=false
            shift
            ;;
        --cli-only)
            INSTALL_SERVER=false
            INSTALL_CLI=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--server-only] [--cli-only] [--help]"
            echo ""
            echo "Options:"
            echo "  --server-only    Install only the server component"
            echo "  --cli-only       Install only the CLI client"
            echo "  --help, -h       Show this help message"
            echo ""
            echo "If no option is specified, both server and CLI will be installed."
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Use '$0 --help' for usage information"
            exit 1
            ;;
    esac
done

# Configuration
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/openzt-instance-manager"
SERVICE_NAME="openzt-instance-manager"

# Check for sudo command
if ! command -v sudo &> /dev/null; then
    echo -e "${RED}Error: sudo is required but not found${NC}"
    echo "Please install sudo first"
    exit 1
fi

echo -e "${GREEN}OpenZT Instance Manager Installer${NC}"
echo "=================================="

# Check for Docker
if ! command -v docker &> /dev/null; then
    echo -e "${YELLOW}Warning: Docker not found. Please install Docker first.${NC}"
    echo "Visit: https://docs.docker.com/get-docker/"
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo -e "${YELLOW}Warning: Docker daemon is not running. Please start Docker first.${NC}"
    exit 1
fi

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Cargo not found. Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Find the workspace root by looking for Cargo.toml with [workspace]
find_workspace_root() {
    local dir="$1"
    while [[ "$dir" != "/" ]]; do
        if [[ -f "$dir/Cargo.toml" ]] && grep -q '^\[workspace\]' "$dir/Cargo.toml" 2>/dev/null; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}

# Get the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Find workspace root
WORKSPACE_ROOT="$(find_workspace_root "$SCRIPT_DIR")"
if [[ -n "$WORKSPACE_ROOT" ]]; then
    echo -e "${GREEN}Found workspace root: $WORKSPACE_ROOT${NC}"
    cd "$WORKSPACE_ROOT"
else
    echo -e "${YELLOW}No workspace found, building from: $SCRIPT_DIR${NC}"
    cd "$SCRIPT_DIR"
fi

echo ""
echo -e "${GREEN}Building binaries...${NC}"
echo "Working directory: $(pwd)"

# Build with explicit path
if [[ "$INSTALL_SERVER" == true ]]; then
    cargo build --release --bin openzt-instance-manager
fi
if [[ "$INSTALL_CLI" == true ]]; then
    cargo build --release --bin openzt --features cli
fi

# Find the target directory (could be in workspace root or local)
TARGET_DIR=""
if [[ "$INSTALL_SERVER" == true && -f "target/release/openzt-instance-manager" ]] || \
   [[ "$INSTALL_SERVER" == false && "$INSTALL_CLI" == true && -f "target/release/openzt" ]]; then
    TARGET_DIR="$(pwd)/target"
elif [[ "$INSTALL_SERVER" == true && -f "$SCRIPT_DIR/target/release/openzt-instance-manager" ]] || \
     [[ "$INSTALL_SERVER" == false && "$INSTALL_CLI" == true && -f "$SCRIPT_DIR/target/release/openzt" ]]; then
    TARGET_DIR="$SCRIPT_DIR/target"
else
    echo -e "${RED}Error: Cannot find built binaries${NC}"
    echo "Searched in:"
    echo "  - $(pwd)/target/"
    echo "  - $SCRIPT_DIR/target/"
    echo ""
    echo "Please check if the build succeeded."
    exit 1
fi

echo ""
echo -e "${GREEN}Installing binaries from $TARGET_DIR${NC}"

# Verify binaries exist before copying
if [[ "$INSTALL_SERVER" == true && ! -f "$TARGET_DIR/release/openzt-instance-manager" ]]; then
    echo -e "${RED}Error: openzt-instance-manager binary not found${NC}"
    exit 1
fi
if [[ "$INSTALL_CLI" == true && ! -f "$TARGET_DIR/release/openzt" ]]; then
    echo -e "${RED}Error: openzt binary not found${NC}"
    exit 1
fi

# Install binaries
if [[ "$INSTALL_SERVER" == true ]]; then
    sudo cp "$TARGET_DIR/release/openzt-instance-manager" "$INSTALL_DIR/"
    sudo chmod +x "$INSTALL_DIR/openzt-instance-manager"
    echo -e "${GREEN}Installed: $INSTALL_DIR/openzt-instance-manager${NC}"
fi
if [[ "$INSTALL_CLI" == true ]]; then
    sudo cp "$TARGET_DIR/release/openzt" "$INSTALL_DIR/"
    sudo chmod +x "$INSTALL_DIR/openzt"
    echo -e "${GREEN}Installed: $INSTALL_DIR/openzt${NC}"
fi

echo ""
echo -e "${GREEN}Creating config directory...${NC}"
sudo mkdir -p "$CONFIG_DIR"

# Copy or create config file (look in script directory)
CONFIG_SOURCE=""
if [[ -f "$SCRIPT_DIR/config.toml" ]]; then
    CONFIG_SOURCE="$SCRIPT_DIR/config.toml"
elif [[ -f "config.toml" ]]; then
    CONFIG_SOURCE="$(pwd)/config.toml"
fi

if [[ -n "$CONFIG_SOURCE" ]]; then
    sudo cp "$CONFIG_SOURCE" "$CONFIG_DIR/"
    echo "Copied config from: $CONFIG_SOURCE"
else
    echo "Warning: config.toml not found, will use defaults"
fi

echo ""
echo -e "${GREEN}Creating systemd service...${NC}"

if [[ "$INSTALL_SERVER" == true ]]; then
    # Create systemd service file using sudo tee
    sudo tee /etc/systemd/system/$SERVICE_NAME.service > /dev/null << EOF
[Unit]
Description=OpenZT Instance Manager API Server
After=docker.service network-online.target
Requires=docker.service

[Service]
Type=simple
User=root
WorkingDirectory=$CONFIG_DIR
ExecStart=$INSTALL_DIR/openzt-instance-manager
Restart=always
RestartSec=10

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=openzt-instance-manager

# Security
NoNewPrivileges=false
# PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

    # Reload systemd
    sudo systemctl daemon-reload
    echo -e "${GREEN}Systemd service created: /etc/systemd/system/$SERVICE_NAME.service${NC}"
else
    echo -e "${YELLOW}Skipped: systemd service creation (server not requested)${NC}"
fi

echo ""
echo -e "${GREEN}Installation complete!${NC}"
echo ""

# Show what was installed
echo "Binaries installed:"
if [[ "$INSTALL_SERVER" == true ]]; then
    echo "  - $INSTALL_DIR/openzt-instance-manager (server)"
fi
if [[ "$INSTALL_CLI" == true ]]; then
    echo "  - $INSTALL_DIR/openzt (client)"
fi
echo ""

# Show config directory info
if [[ "$INSTALL_SERVER" == true ]]; then
    echo "Config directory: $CONFIG_DIR"
    echo ""
    echo "To start the server:"
    echo "  sudo systemctl start $SERVICE_NAME"
    echo ""
    echo "To enable the server to start on boot:"
    echo "  sudo systemctl enable $SERVICE_NAME"
    echo ""
    echo "To check server status:"
    echo "  sudo systemctl status $SERVICE_NAME"
    echo ""
    echo "To view logs:"
    echo "  sudo journalctl -u $SERVICE_NAME -f"
    echo ""
fi

# Show CLI usage if CLI was installed
if [[ "$INSTALL_CLI" == true ]]; then
    echo -e "${GREEN}CLI usage examples:${NC}"
    echo "  openzt health                      # Check API health"
    echo "  openzt list                        # List instances"
    echo "  openzt create <path-to-dll>        # Create instance"
    echo "  openzt get <instance-id>           # Get instance details"
    echo "  openzt logs <instance-id>          # Get instance logs"
    echo "  openzt delete <instance-id>        # Delete instance"
    echo ""
    echo "Use 'openzt --help' for more information."
    echo ""
fi

# Show helpful note for CLI-only installs
if [[ "$INSTALL_SERVER" == false && "$INSTALL_CLI" == true ]]; then
    echo -e "${YELLOW}Note: Only the CLI was installed.${NC}"
    echo "The CLI requires a running server. Make sure the server is installed and running"
    echo "on a reachable host, or set the OPENZT_API_URL environment variable:"
    echo "  export OPENZT_API_URL=http://your-server:3000"
    echo ""
fi
