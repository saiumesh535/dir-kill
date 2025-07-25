# Project Brief: dir-kill

## Project Overview
`dir-kill` is a sophisticated Rust-based directory management tool that provides advanced directory listing, searching, and selection capabilities through both command-line interface (CLI) and terminal user interface (TUI). The project has achieved production-ready status with comprehensive testing and optimization.

## Core Requirements

### Primary Functionality
1. **Directory Pattern Matching**: Find directories matching specific patterns (e.g., `node_modules`)
2. **Real-time Scanning**: Display directories as they are discovered during scanning
3. **Size Calculation**: Calculate and display directory sizes with lazy loading
4. **Interactive Selection**: Multi-select directories with visual feedback
5. **Beautiful TUI**: Rich terminal interface with animations and color themes
6. **Progressive Visual Deletion**: Real-time feedback during directory deletion operations

### Technical Requirements
- **Language**: Rust
- **CLI Framework**: clap for argument parsing
- **TUI Library**: ratatui with crossterm backend
- **Error Handling**: anyhow for human-readable errors
- **Testing**: Comprehensive TDD approach with tempfile for isolated testing

### User Experience Goals
- **Intuitive Interface**: Clear visual indicators and smooth animations
- **Performance**: Real-time updates without blocking the UI
- **Accessibility**: Fallback to text mode for unsupported terminals
- **Responsiveness**: Immediate feedback for user actions

## Key Features Implemented

### 1. CLI Interface
- `ls` command with pattern matching
- Support for various flags (--long, --all, etc.)
- Path specification and pattern matching
- Help system integration

### 2. TUI Interface
- **Gruvbox Dark Theme**: Beautiful color palette
- **Animated Directory Icons**: Open/closed states with selection indicators
- **Real-time Scanning**: Live updates during directory discovery
- **Pagination**: Dynamic page sizing based on viewport
- **Multi-panel Layout**: Directory list + details panel

### 3. Selection System
- **Multi-select Capability**: Select individual or all directories
- **Visual Indicators**: Animated checkmarks and directory icons
- **Selection Summary**: Count and total size of selected items
- **Keyboard Shortcuts**: Space, A, D, S for selection operations

### 4. Size Calculation
- **Lazy Loading**: Immediate display with background size calculation
- **Progress Tracking**: Shows calculated vs total items
- **Size Formatting**: Human-readable size display (B, KB, MB, etc.)
- **Total Size Display**: Sum of all directories

### 5. Delete Operations
- **Progressive Visual Feedback**: Real-time status updates during deletion
- **Multiple Key Combinations**: F/x/Ctrl+D for current, X/Ctrl+Shift+D for selected
- **Status Indicators**: Normal, Deleting, Deleted, and Error states with color coding
- **Safe Operations**: Multiple confirmation methods to prevent accidental deletions

### 6. Error Handling & Fallbacks
- **Terminal Detection**: Automatic fallback for unsupported terminals
- **Graceful Degradation**: Text mode when TUI fails
- **Error Recovery**: Robust error handling throughout

## Architecture Principles
- **Modular Design**: Separate modules for CLI, UI, and file system operations
- **Test-Driven Development**: Comprehensive test coverage (87 tests)
- **Performance First**: Non-blocking operations and efficient algorithms
- **User-Centric**: Focus on usability and visual appeal

## Success Criteria
- [x] Functional CLI with pattern matching
- [x] Beautiful TUI with animations
- [x] Real-time directory scanning
- [x] Multi-select functionality
- [x] Lazy size calculation
- [x] Progressive visual deletion with real-time feedback
- [x] Comprehensive test coverage (87 tests)
- [x] Terminal compatibility and fallbacks
- [x] Performance optimization
- [x] Production-ready stability 