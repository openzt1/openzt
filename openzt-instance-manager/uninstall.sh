#!/bin/bash
# OpenZT Instance Manager Uninstallation Script

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/openzt-instance-manager"
SERVICE_NAME="openzt-instance-manager"

# Check for sudo command
if ! command -v sudo &> /dev/null; then
    echo -e "${RED}Error: sudo is required but not found${NC}"
    echo "Please install sudo first"
    exit 1
fi

echo -e "${RED}OpenZT Instance Manager Uninstaller${NC}"
echo "====================================="
echo ""

# Stop and disable service
if sudo systemctl is-active --quiet $SERVICE_NAME 2>/dev/null; then
    echo -e "${YELLOW}Stopping $SERVICE_NAME service...${NC}"
    sudo systemctl stop $SERVICE_NAME
fi

if sudo systemctl is-enabled --quiet $SERVICE_NAME 2>/dev/null; then
    echo -e "${YELLOW}Disabling $SERVICE_NAME service...${NC}"
    sudo systemctl disable $SERVICE_NAME
fi

# Remove systemd service
echo -e "${YELLOW}Removing systemd service...${NC}"
sudo rm -f /etc/systemd/system/$SERVICE_NAME.service
sudo systemctl daemon-reload

# Remove binaries
echo -e "${YELLOW}Removing binaries...${NC}"
sudo rm -f "$INSTALL_DIR/openzt-instance-manager"
sudo rm -f "$INSTALL_DIR/openzt"

echo ""
echo -e "${GREEN}Uninstallation complete!${NC}"
echo ""
echo "The following have been removed:"
echo "  - $INSTALL_DIR/openzt-instance-manager"
echo "  - $INSTALL_DIR/openzt"
echo "  - /etc/systemd/system/$SERVICE_NAME.service"
echo ""
echo "Note: Config directory $CONFIG_DIR was preserved."
echo "To remove it manually:"
echo "  sudo rm -rf $CONFIG_DIR"
