# TODO: dir-kill btop-style Enhancements

## 🎯 Vision: Transform dir-kill into a btop-style Disk Space Analyzer

Transform the current functional directory management tool into a **powerful disk space analyzer** with btop's sophisticated approach to data visualization, search, filtering, and sorting.

**Important**: Core search and delete functionality remains unchanged - we're adding visual enhancements and UI improvements only.

## 📋 Task Categories

### 🔥 High Priority - Core btop Features

#### 1. Search & Filtering System (Visual Enhancements)
- [ ] **Real-time Search Bar**
  - [ ] Add search input field at top of TUI
  - [ ] Implement real-time filtering as user types (visual only)
  - [ ] Support for simple text search (case-insensitive)
  - [ ] Add search history (last 10 searches)
  - [ ] Clear search functionality (Ctrl+L or Escape)
  - [ ] Visual search indicators and highlighting

- [ ] **Advanced Filtering UI**
  - [ ] Size range filters (0-1MB, 1-10MB, 10-100MB, 100MB+)
  - [ ] Date range filters (last modified, created)
  - [ ] Path pattern filters (contains, starts with, ends with)
  - [ ] Filter combination UI (AND/OR logic)
  - [ ] Quick filter presets (Large files, Recent, Hidden)

#### 2. Enhanced Sorting & Organization
- [ ] **Multi-column Sorting**
  - [ ] Sort by name (alphabetical)
  - [ ] Sort by size (largest/smallest first)
  - [ ] Sort by last modified date
  - [ ] Sort by creation date
  - [ ] Sort by path depth
  - [ ] Visual sort indicators (arrows, active column highlighting)

- [ ] **Grouping Options**
  - [ ] Group by size ranges
  - [ ] Group by date ranges
  - [ ] Group by file type/extension
  - [ ] Collapsible groups with summary stats

#### 3. Advanced Data Visualization
- [ ] **Size Distribution Charts**
  - [ ] Horizontal bar charts for directory sizes
  - [ ] Pie charts for size distribution
  - [ ] Treemap visualization
  - [ ] Size histogram

- [ ] **Progress & Statistics**
  - [ ] Real-time scanning progress with detailed stats
  - [ ] Total size, count, and average size displays
  - [ ] Scanning speed and ETA indicators
  - [ ] Memory usage and performance metrics

### 🟡 Medium Priority - Enhanced UX

#### 4. Improved Navigation & Controls
- [ ] **Keyboard Shortcuts Enhancement**
  - [ ] Search mode toggle (Ctrl+F)
  - [ ] Sort mode cycling (Tab)
  - [ ] Filter mode toggle (Ctrl+Shift+F)
  - [ ] View mode switching (Ctrl+V)
  - [ ] Help overlay (F1)

- [ ] **Context Menus**
  - [ ] Right-click context menus for directories
  - [ ] Quick actions (open, copy path, show in finder)
  - [ ] Bulk operations menu
  - [ ] Settings and preferences

#### 5. Real-time Updates & Animations
- [ ] **Live Data Updates**
  - [ ] Real-time size recalculation
  - [ ] Live filtering as data changes
  - [ ] Smooth transitions between states
  - [ ] Loading animations and spinners

- [ ] **Visual Feedback**
  - [ ] Hover effects and tooltips
  - [ ] Selection animations
  - [ ] Delete progress animations
  - [ ] Error state animations

### 🟢 Low Priority - Polish & Advanced Features

#### 6. Advanced Visualizations
- [ ] **Heat Maps**
  - [ ] Size-based color coding
  - [ ] Age-based color coding
  - [ ] Custom color schemes
  - [ ] Legend and color scale

- [ ] **3D Visualizations**
  - [ ] 3D directory tree
  - [ ] Interactive 3D navigation
  - [ ] Zoom and pan controls

#### 7. Customization & Themes
- [ ] **Theme System**
  - [ ] Multiple color themes (dark, light, custom)
  - [ ] Font customization
  - [ ] Layout customization
  - [ ] Animation speed controls

- [ ] **User Preferences**
  - [ ] Default sort order
  - [ ] Default view mode
  - [ ] Keyboard shortcut customization
  - [ ] Performance settings

## 🚀 Implementation Strategy

### Phase 1: Foundation (Weeks 1-2)
1. **Search Bar Implementation**
   - Add search input field to TUI
   - Implement real-time filtering UI
   - Add search history and clear functionality

2. **Basic Sorting UI**
   - Add sort controls to TUI
   - Implement visual sort indicators
   - Add keyboard shortcuts for sorting

### Phase 2: Enhanced Visualization (Weeks 3-4)
1. **Size Distribution Charts**
   - Implement horizontal bar charts
   - Add size statistics display
   - Create progress indicators

2. **Advanced Filtering UI**
   - Add filter controls
   - Implement filter combination logic
   - Add quick filter presets

### Phase 3: Advanced Features (Weeks 5-6)
1. **Grouping and Organization**
   - Implement grouping options
   - Add collapsible groups
   - Create group summary displays

2. **Real-time Updates**
   - Implement live data updates
   - Add smooth transitions
   - Create loading animations

### Phase 4: Polish & Optimization (Weeks 7-8)
1. **Performance Optimization**
   - Optimize rendering performance
   - Implement efficient data structures
   - Add memory management

2. **User Experience Polish**
   - Add context menus
   - Implement tooltips
   - Create help system

### Phase 5: Advanced Features (Weeks 9-10)
1. **Advanced Visualizations**
   - Implement heat maps
   - Add custom themes
   - Create 3D visualizations

2. **Customization**
   - Add user preferences
   - Implement theme system
   - Create configuration options

## 📊 Success Metrics

### Performance Targets
- **UI Responsiveness**: Maintain 60fps rendering
- **Search Speed**: Real-time filtering < 16ms
- **Memory Usage**: < 100MB for large directories
- **Startup Time**: < 2 seconds

### User Experience Goals
- **Intuitive Navigation**: New users can use advanced features within 5 minutes
- **Visual Appeal**: Beautiful, modern interface that rivals btop
- **Efficiency**: Complete common tasks 50% faster than current version
- **Accessibility**: Full keyboard navigation and screen reader support

## 🔧 Technical Considerations

### Architecture Changes
- **UI Layer Only**: No changes to core fs/ logic
- **State Management**: Enhance App struct for new UI features
- **Rendering**: Optimize ratatui widget rendering
- **Data Flow**: Maintain existing data structures

### Dependencies
- **No New Dependencies**: Use existing ratatui capabilities
- **Performance**: Optimize existing code for new features
- **Testing**: Maintain 100% test coverage for core functionality

### Compatibility
- **Terminal Support**: Maintain cross-platform compatibility
- **Backward Compatibility**: All existing functionality preserved
- **Configuration**: Maintain existing config file format 