# Product Context: dir-kill

## Why This Project Exists

### Problem Statement
Developers and system administrators frequently need to:
1. **Find specific directory patterns** across large codebases (e.g., `node_modules`, `target`, `build`)
2. **Analyze directory sizes** to identify space-consuming directories
3. **Select multiple directories** for bulk operations (deletion, backup, analysis)
4. **Navigate complex directory structures** with visual feedback

Traditional tools like `find` and `ls` lack:
- Real-time visual feedback during scanning
- Interactive selection capabilities
- Beautiful, modern interfaces
- Size analysis with progress tracking

### Target Users
- **Developers**: Managing large codebases with multiple dependency directories
- **System Administrators**: Analyzing disk usage and identifying large directories
- **DevOps Engineers**: Cleaning up build artifacts and temporary files
- **Power Users**: Anyone needing advanced directory management capabilities

## How It Should Work

### User Workflow
1. **Command Execution**: User runs `dir-kill ls node_modules` or similar
2. **Pattern Matching**: Tool searches for directories matching the pattern
3. **Real-time Display**: TUI shows directories as they're found with loading animations
4. **Interactive Selection**: User can select directories using keyboard shortcuts
5. **Size Analysis**: Directory sizes are calculated in the background
6. **Visual Feedback**: Beautiful animations and color-coded information

### Key User Experience Principles

#### 1. Immediate Feedback
- **Loading Animations**: Spinning indicators during scanning
- **Real-time Updates**: Directories appear as they're discovered
- **Progress Indicators**: Shows scanning progress and completion status

#### 2. Intuitive Interface
- **Visual Hierarchy**: Clear distinction between different UI elements
- **Color Coding**: Consistent color scheme for different states
- **Animated Elements**: Subtle animations for selection and loading states

#### 3. Performance
- **Non-blocking Operations**: UI remains responsive during heavy operations
- **Lazy Loading**: Size calculations happen in background
- **Efficient Scanning**: Optimized directory traversal algorithms

#### 4. Accessibility
- **Terminal Compatibility**: Works across different terminal types
- **Fallback Modes**: Text output when TUI isn't supported
- **Keyboard Navigation**: Full keyboard control without mouse dependency

## User Stories

### Primary User Stories
1. **As a developer**, I want to find all `node_modules` directories in my project so I can clean up disk space
2. **As a sysadmin**, I want to identify the largest directories on a system so I can manage storage
3. **As a DevOps engineer**, I want to select multiple build directories for cleanup so I can automate maintenance
4. **As a power user**, I want to see real-time scanning progress so I know the tool is working

### Secondary User Stories
1. **As a user**, I want beautiful visual feedback so the tool feels modern and polished
2. **As a user**, I want keyboard shortcuts so I can work efficiently
3. **As a user**, I want size information so I can make informed decisions
4. **As a user**, I want pagination so I can navigate large result sets

## Success Metrics
- **Usability**: Users can find and select directories quickly
- **Performance**: Scanning completes within reasonable time for large directories
- **Reliability**: Tool works consistently across different environments
- **Aesthetics**: Interface is visually appealing and modern
- **Accessibility**: Works on various terminal types and configurations

## Competitive Advantages
- **Real-time Feedback**: Unlike traditional CLI tools
- **Interactive Selection**: Multi-select capabilities not available in basic tools
- **Modern Interface**: Beautiful TUI with animations
- **Performance**: Optimized for large directory structures
- **Developer Experience**: Built with modern development practices 