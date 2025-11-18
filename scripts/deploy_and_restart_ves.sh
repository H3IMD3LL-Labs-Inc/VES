#!/usr/bin/env bash
set -e

# Paths
DEPLOY_DIR="$HOME/ves"
NEW_BINARY="$DEPLOY_DIR/ves.new"
TARGET_BINARY="/usr/local/bin/ves"
BACKUP_DIR="$DEPLOY_DIR/backups"

# Ensure backup directory exists
mkdir -p "$BACKUP_DIR"

# Backup existing binary if it exists
if [ -f "$TARGET_BINARY" ]; then
    TIMESTAMP=$(date +%Y%m%d%H%M%S)
    echo "Backing up existing binary to $BACKUP_DIR/ves.$TIMESTAMP"
    mv "$TARGET_BINARY" "$BACKUP_DIR/ves.$TIMESTAMP"
fi

# Move new binary into place
echo "Deploying new binary to $TARGET_BINARY"
sudo mv "$NEW_BINARY" "$TARGET_BINARY"
sudo chmod +x "$TARGET_BINARY"

echo "Deployment complete..."
echo "You can now run 'ves run' to run the new binary..."
