# Technical Context: dir-kill

## Technology Stack

### Core Technologies
- **Language**: Rust (latest stable)
- **Package Manager**: Cargo
- **Build System**: Cargo (Rust's built-in build system)

### Key Dependencies

#### CLI Framework
```toml
clap = { version = "4.0", features = ["derive"] }
```
- **Purpose**: Command-line argument parsing and help generation
- **Features**: Derive macros for automatic CLI generation
- **Benefits**: Industry standard, excellent error messages, auto-completion support

#### TUI Framework
```toml
ratatui = "0.24"
crossterm = "0.27"
```
- **ratatui**: Modern terminal UI library with excellent performance
- **crossterm**: Cross-platform terminal manipulation backend
- **Benefits**: Rich widget system, efficient rendering, cross-platform compatibility

#### Error Handling
```toml
anyhow = "1.0"
```
- **Purpose**: Human-readable error handling and propagation
- **Benefits**: Automatic error conversion, context preservation, user-friendly messages

#### Testing
```toml
tempfile = "3.8"
```
- **Purpose**: Isolated testing environments
- **Benefits**: Automatic cleanup, cross-platform compatibility, safe testing

#### System Integration
```toml
libc = "0.2"
```
- **Purpose**: Low-level system calls for terminal detection
- **Benefits**: Cross-platform terminal capability detection

## Development Setup

### Prerequisites
- **Rust**: Latest stable version (1.70+)
- **Cargo**: Included with Rust installation
- **Terminal**: Any modern terminal emulator
- **Git**: For version control

### Environment Requirements
- **Operating System**: macOS, Linux, Windows (with WSL recommended)
- **Terminal**: Supports UTF-8 and color output
- **Memory**: Minimal requirements (16MB+ for large directory scans)
- **Disk**: Minimal space for binary and temporary files

### Build Configuration
```toml
[package]
name = "dir-kill"
version = "0.1.0"
edition = "2021"

[profile.dev]
opt-level = 0
debug = true

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

## Technical Constraints

### Performance Constraints
- **Real-time UI**: Must maintain 60fps rendering during scanning
- **Memory Usage**: Efficient handling of large directory structures
- **CPU Usage**: Background operations must not block UI thread
- **I/O Performance**: Optimized file system operations

### Platform Constraints
- **Terminal Compatibility**: Must work across different terminal types
- **Unicode Support**: Full UTF-8 support for international characters
- **Color Support**: Graceful fallback for monochrome terminals
- **Window Size**: Responsive to terminal window resizing

### Security Constraints
- **Path Validation**: Prevent directory traversal attacks
- **File System Safety**: Safe operations on user-specified paths
- **Memory Safety**: Rust's ownership system prevents common vulnerabilities
- **Error Handling**: No information leakage through error messages

## Architecture Constraints

### Module Dependencies
```
main.rs
├── cli/ (independent)
├── fs/ (independent)
└── ui/ (depends on fs, independent of cli)
```

### Threading Model
- **Main Thread**: UI rendering and event handling
- **Background Threads**: File system operations and size calculations
- **Communication**: Thread-safe channels for data exchange
- **Synchronization**: Minimal shared state, message-passing architecture

### Error Handling Strategy
- **Propagation**: Use `anyhow::Result<T>` for error propagation
- **Context**: Add context to errors for better debugging
- **Recovery**: Graceful fallbacks for recoverable errors
- **Reporting**: User-friendly error messages

## Testing Strategy

### Test Types
1. **Unit Tests**: Individual function testing
2. **Integration Tests**: Module interaction testing
3. **Property Tests**: Automated property-based testing
4. **Performance Tests**: Benchmarking critical paths

### Test Environment
- **Isolation**: Use `tempfile` for isolated test environments
- **Mocking**: Minimal mocking, prefer real file system operations
- **Coverage**: Aim for >90% code coverage
- **CI/CD**: Automated testing on multiple platforms

### Test Patterns
```rust
#[test]
fn test_with_temp_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Test operations in isolated environment
    // Automatic cleanup on test completion
}
```

## Performance Characteristics

### Time Complexity
- **Directory Scanning**: O(n) where n is number of directories
- **Size Calculation**: O(m) where m is total file count
- **UI Rendering**: O(k) where k is visible items
- **Search Operations**: O(n) for pattern matching

### Space Complexity
- **Directory List**: O(n) for storing directory information
- **Size Cache**: O(n) for calculated sizes
- **UI State**: O(1) for application state
- **Temporary Files**: O(1) for test isolation

### Memory Usage
- **Baseline**: ~2-5MB for basic operation
- **Large Scans**: Scales linearly with directory count
- **Size Calculation**: Minimal additional memory for background operations
- **UI Rendering**: Efficient widget reuse and minimal allocations

## Deployment Considerations

### Binary Distribution
- **Single Binary**: Statically linked executable
- **Cross-compilation**: Support for multiple target platforms
- **Size Optimization**: Release builds with LTO and optimization
- **Dependencies**: No external runtime dependencies

### Installation Methods
- **Cargo Install**: `cargo install dir-kill`
- **Manual Build**: `cargo build --release`
- **Package Managers**: Future support for system package managers

### Runtime Requirements
- **Terminal**: Any modern terminal emulator
- **File System**: Standard file system access
- **Permissions**: Read access to scanned directories
- **Network**: None required (offline operation)

## Monitoring and Debugging

### Logging Strategy
- **Error Logging**: Structured error reporting
- **Performance Metrics**: Timing for critical operations
- **Debug Information**: Configurable debug output
- **User Feedback**: Progress indicators and status messages

### Debugging Tools
- **Rust Debugger**: Integration with rust-gdb/rust-lldb
- **Profiling**: Integration with perf and flamegraph
- **Memory Analysis**: Valgrind and sanitizer support
- **Terminal Debugging**: Raw mode debugging capabilities

## Future Technical Considerations

### Scalability
- **Large Directory Support**: Efficient handling of 100k+ directories
- **Memory Optimization**: Streaming for very large result sets
- **Parallel Processing**: Multi-threaded scanning for large directories
- **Caching**: Persistent cache for repeated operations

### Extensibility
- **Plugin System**: Future support for custom commands
- **Configuration**: User-configurable settings and themes
- **API**: Library interface for programmatic use
- **Integration**: IDE and editor plugin support 