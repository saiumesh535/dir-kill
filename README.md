# dir-kill 🗂️

A Rust-based directory management tool for finding, inspecting, and deleting directories that match a pattern. It launches an interactive terminal UI (TUI) with real-time scanning, size calculation, and multi-select deletion.

## What it does

dir-kill scans your file system for directories matching a name pattern (for example `node_modules`, `target`, or `dist`). Results appear live in a Gruvbox-themed TUI with directory sizes, pagination, and keyboard-driven selection. You can delete individual directories or bulk-delete selected ones to reclaim disk space.

On terminals that do not support the TUI (for example macOS Terminal.app or `TERM=dumb`), dir-kill falls back to plain-text output.

<img src="./assets/dir-kill.png" alt="dir-kill screenshot" width="800" height="600" style="max-width: 100%; height: auto; border-radius: 8px; box-shadow: 0 4px 8px rgba(0,0,0,0.1);">

## Features

- **Real-time discovery** — directories stream in as they are found
- **Background size calculation** — sizes are computed without blocking the UI
- **Multi-select** — mark directories with Space, then delete in bulk
- **Parallel deletion** — background workers remove directories with live progress
- **Ignore patterns** — skip directories using comma-separated regex patterns
- **Nested pattern avoidance** — does not recurse into matched directories (see below)
- **Text fallback** — simple listing when the TUI is unavailable

## Installation

### From source

```bash
git clone https://github.com/saiumesh535/dir-kill.git
cd dir-kill
cargo install --path .
```

Or build without installing:

```bash
cargo build --release
./target/release/dir-kill ls node_modules
```

### From a release

1. Download the latest release from [GitHub Releases](https://github.com/saiumesh535/dir-kill/releases)
2. `chmod +x dir-kill`
3. `mv dir-kill /usr/local/bin/dir-kill`

## Usage

```bash
dir-kill ls <pattern> [path] [OPTIONS]
```

| Argument / Option | Description |
|---|---|
| `pattern` | Directory name to match (e.g. `node_modules`) |
| `path` | Root directory to scan (default: `.`) |
| `-i`, `--ignore <PATTERNS>` | Comma-separated regex patterns for directories to skip |

### Examples

```bash
# Find node_modules directories under the current directory
dir-kill ls node_modules

# Scan a specific path
dir-kill ls target ~/projects

# Ignore .git and temp directories
dir-kill ls node_modules --ignore "\.git,temp"

# Multiple ignore patterns
dir-kill ls dist -i "node_modules,\.git,backup"
```

## TUI keyboard shortcuts

| Key | Action |
|---|---|
| `↑` / `↓` / `j` / `k` | Move selection |
| `←` / `→` | Previous / next page |
| `Home` / `End` | Jump to first / last item |
| `Space` | Toggle selection on current item |
| `a` | Select all |
| `d` | Deselect all |
| `F` | Delete current directory |
| `Ctrl+D` / `Ctrl+X` | Delete current directory |
| `C` | Delete selected directories |
| `q` / `Esc` | Quit |

## Nested pattern avoidance

dir-kill automatically skips scanning inside directories that already match the search pattern. This prevents redundant results and improves performance.

```bash
# Finds /project/node_modules but not /project/node_modules/some-package/node_modules
dir-kill ls node_modules

# Finds /project/dist but not /project/dist/build/dist
dir-kill ls dist
```

## Development

```bash
# Run tests
make test
# or
cargo test

# Build release binary
make build
```

### Project structure

```
src/
├── main.rs   # Entry point
├── cli/      # Command-line argument parsing (clap)
├── fs/       # Directory scanning, sizing, and deletion
└── ui/       # TUI rendering and interaction (ratatui)
```