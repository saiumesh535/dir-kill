# System Patterns: dir-kill

## Architecture Overview

### High-Level Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   CLI Module    │    │   TUI Module    │    │   FS Module     │
│   (clap)        │    │   (ratatui)     │    │   (std::fs)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   App State     │
                    │   Management    │
                    └─────────────────┘
```

### Module Structure
```
src/
├── main.rs          # Entry point, delegates to modules
├── cli/
│   ├── mod.rs       # CLI structure and argument parsing
│   └── tests.rs     # CLI unit tests
├── fs/
│   ├── mod.rs       # File system utilities
│   └── tests.rs     # FS unit tests
└── ui/
    ├── mod.rs       # TUI rendering and event handling
    ├── app.rs       # Application state management
    └── list.rs      # List widget utilities
```

## Key Technical Decisions

### 1. Rust Ecosystem Choices
- **clap**: Industry-standard CLI argument parsing
- **ratatui**: Modern TUI library with excellent performance
- **crossterm**: Cross-platform terminal manipulation
- **anyhow**: Human-readable error handling
- **tempfile**: Isolated testing environments

### 2. Architecture Patterns

#### Module Separation
- **CLI Module**: Handles argument parsing and command routing
- **FS Module**: File system operations and directory traversal
- **UI Module**: TUI rendering, event handling, and state management

#### State Management
- **App Struct**: Central state container for TUI data
- **Immutable Updates**: State changes through method calls
- **Event-Driven**: UI updates based on user input and background events

#### Concurrency Model
- **Background Threads**: Directory scanning and size calculation
- **Channel Communication**: Thread-safe data exchange
- **Non-blocking UI**: Main thread remains responsive

### 3. Design Patterns

#### Observer Pattern
- Background threads send updates via channels
- UI thread observes and renders changes
- Real-time updates without blocking

#### Command Pattern
- CLI commands map to specific functions
- Consistent interface for different operations
- Easy to extend with new commands

#### Builder Pattern
- TUI widgets constructed with fluent interfaces
- Complex UI layouts built incrementally
- Readable and maintainable UI code

## Component Relationships

### Data Flow
```
User Input → CLI Parser → Command Handler → FS Operations → Background Threads
                                                              ↓
UI Thread ← Channel Updates ← State Updates ← Directory Data
```

### State Management Flow
```
App State → UI Rendering → User Events → State Updates → UI Re-rendering
```

### Concurrency Flow
```
Main Thread (UI) ←→ Channels ←→ Background Threads (Scanning/Calculation)
```

## Key Implementation Patterns

### 1. Error Handling Strategy
```rust
// Consistent error handling with anyhow
pub fn some_operation() -> Result<()> {
    // Operations that can fail
    Ok(())
}
```

### 2. Testing Strategy
```rust
// Isolated testing with tempfile
#[test]
fn test_with_temp_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Test operations in isolated environment
}
```

### 3. UI State Management
```rust
// Centralized state with methods for updates
pub struct App {
    pub directories: Vec<DirectoryInfo>,
    pub selected: usize,
    // ... other fields
}

impl App {
    pub fn toggle_selection_mode(&mut self) {
        self.selection_mode = !self.selection_mode;
    }
}
```

### 4. Background Processing
```rust
// Non-blocking operations with channels
let (tx, rx) = std::sync::mpsc::channel();
std::thread::spawn(move || {
    // Background work
    tx.send(result).unwrap();
});
```

## Performance Considerations

### 1. Lazy Loading
- Directory sizes calculated in background
- UI renders immediately with placeholders
- Progressive updates as calculations complete

### 2. Efficient Scanning
- Recursive directory traversal optimized
- Early termination for nested `node_modules`
- Memory-efficient data structures

### 3. UI Performance
- Minimal re-renders with state-based updates
- Efficient widget construction
- Optimized color and style calculations

## Security and Safety

### 1. Path Safety
- Path validation and sanitization
- Prevention of directory traversal attacks
- Safe file system operations

### 2. Memory Safety
- Rust's ownership system prevents memory leaks
- Bounded data structures prevent unbounded growth
- Safe concurrent access with channels

### 3. Error Recovery
- Graceful handling of file system errors
- Fallback modes for unsupported operations
- Robust error reporting

## Extensibility Patterns

### 1. Command Extension
```rust
// Easy to add new commands
#[derive(Subcommand)]
enum Commands {
    Ls(LsCommand),
    // New commands can be added here
}
```

### 2. UI Extension
```rust
// Modular UI components
pub mod list;
pub mod app;
// New UI modules can be added
```

### 3. Feature Flags
- Conditional compilation for optional features
- Runtime feature detection for terminal capabilities
- Graceful degradation for missing features 