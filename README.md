# dir-kill 🗂️

A Rust-based directory management tool that helps you find and manage directories. It provides terminal user interface for discovering directories matching specific patterns.

## What it does

dir-kill scans your file system to find directories that match specified patterns. It's particularly useful for finding and deleting stale project directories like `node_modules`, `target`, `dist`, and other build artifacts to help clean up disk space.

<img src="./assets/dir-kill.png" alt="dir-kill screenshot" width="800" height="600" style="max-width: 100%; height: auto; border-radius: 8px; box-shadow: 0 4px 8px rgba(0,0,0,0.1);">

## CLI Usage

### Basic Commands

```bash
# Find all node_modules directories
dir-kill ls <pattern>
```

### Command Options

```bash
dir-kill ls <pattern> [OPTIONS]

OPTIONS:
    -h, --help             Show help information
```

### Examples

```bash
# Find node_modules directories
dir-kill ls node_modules
```

## Installation

## From Source

```bash
# Clone and build
git clone https://github.com/saiumesh535/dir-kill.git
cd dir-kill
cargo build --release

# Run directly
cargo run -- ls node_modules
```

## From GitHub Releases

Pre-built binaries are available for Windows, Linux, and macOS on the [GitHub Releases](https://github.com/saiumesh535/dir-kill/releases) page.

### Quick Install

```bash
# Linux/macOS
curl -L https://github.com/saiumesh535/dir-kill/releases/latest/download/dir-kill-linux-x86_64 -o dir-kill
chmod +x dir-kill
./dir-kill ls node_modules

# macOS (Apple Silicon)
curl -L https://github.com/saiumesh535/dir-kill/releases/latest/download/dir-kill-macos-aarch64 -o dir-kill
chmod +x dir-kill
./dir-kill ls node_modules
```

# Development

## GitHub Actions

This project uses a unified GitHub Actions workflow for continuous integration and automated releases:

### Unified Workflow (`.github/workflows/build-and-release.yml`)
- **Triggers**: Every push to `master`/`main`, pull requests, and version tags
- **Testing**: Runs tests on Ubuntu, Windows, and macOS with Rust stable, beta, and nightly
- **Quality Checks**: Runs clippy and format checks
- **Building**: Creates release binaries for:
  - Linux (x86_64)
  - Windows (x86_64)
  - macOS (x86_64)
  - macOS (Apple Silicon)
- **Releasing**: Automatically creates GitHub releases based on version in `Cargo.toml`
- **Security**: Includes SHA256 checksums for verification

### Release Process

1. **Automatic Release**: Every push to `master` creates/updates a release with the current version from `Cargo.toml`
2. **Tagged Release**: Create a git tag (e.g., `v1.0.0`) to create a specific versioned release
3. **Version Management**: Update the version in `Cargo.toml` to trigger new releases

### Example Release Workflow

```bash
# Update version in Cargo.toml
# Push to master - automatic release created
git push origin master

# Or create a specific versioned release
git tag v1.0.0
git push origin v1.0.0
``` 