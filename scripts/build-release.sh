#!/bin/bash

# Build script for dir-kill - mirrors GitHub Actions workflow
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building dir-kill release binaries...${NC}"

# Get version from Cargo.toml
VERSION=$(grep '^version = ' Cargo.toml | cut -d '"' -f2)
echo -e "${GREEN}Version: $VERSION${NC}"

# Create release directory
RELEASE_DIR="release"
mkdir -p $RELEASE_DIR

# Function to build for a specific target
build_target() {
    local target=$1
    local artifact_name=$2
    
    echo -e "${YELLOW}Building for $target...${NC}"
    
    # Install target if not already installed
    rustup target add $target
    
    # Build release binary
    cargo build --release --target $target
    
    # Strip debug symbols (except on Windows)
    if [[ "$target" != *"windows"* ]]; then
        strip target/$target/release/dir-kill
    fi
    
    # Copy binary to release directory
    if [[ "$target" == *"windows"* ]]; then
        cp target/$target/release/dir-kill.exe $RELEASE_DIR/$artifact_name
    else
        cp target/$target/release/dir-kill $RELEASE_DIR/$artifact_name
    fi
    
    # Create checksum
    cd $RELEASE_DIR
    if [[ "$target" == *"windows"* ]]; then
        # Note: certutil is Windows-specific, this is for reference
        echo "SHA256 checksum for $artifact_name:" > $artifact_name.sha256
        sha256sum $artifact_name >> $artifact_name.sha256
    else
        sha256sum $artifact_name > $artifact_name.sha256
    fi
    cd ..
    
    echo -e "${GREEN}✓ Built $artifact_name${NC}"
}

# Build for all targets
build_target "x86_64-unknown-linux-gnu" "dir-kill-linux-x86_64"
build_target "x86_64-pc-windows-msvc" "dir-kill-windows-x86_64.exe"
build_target "x86_64-apple-darwin" "dir-kill-macos-x86_64"
build_target "aarch64-apple-darwin" "dir-kill-macos-aarch64"

echo -e "${GREEN}✓ All builds completed!${NC}"
echo -e "${BLUE}Release files in $RELEASE_DIR/:${NC}"
ls -la $RELEASE_DIR/

echo -e "${YELLOW}To create a GitHub release:${NC}"
echo "1. Update version in Cargo.toml"
echo "2. Commit and push to master"
echo "3. Or create a tag: git tag v$VERSION && git push origin v$VERSION" 