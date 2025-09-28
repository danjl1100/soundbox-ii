# Bucket-Spigot Network Visualizer - Requirements

## Overview
Visual interface for bucket-spigot network with read-only display and placeholder editing UI elements.

## Data Model Understanding
- **Network**: Hierarchical tree with spigot (root), joints (intermediate), buckets (leaf)
- **Path**: String addressing (e.g., ".0", ".0.1", ".4.2")
- **TableView**: Pre-calculated layout data from bucket-spigot TypeScript bindings
- **Node Properties**: active status, weights, order types (InOrder/Random/Shuffle)

## Feature Requirements

### 1. Network Display

#### Layout Strategy
- **Depth-Based Columns**: X-axis represents tree depth (row index in TableView), progressing left to right
- **Bucket-Centered Vertical Distribution**: Y-axis represents position within each depth level
- **Display Width Integration**: Node height spans based on `display_width` property for visual hierarchy
- **Reference Implementation**: Follow `bucket-spigot/examples/simple-html/main.rs::write_view_html_svg()` layout calculations

#### Node Visualization
- **Spigot (Root)**: Distinctive central node treatment
- **Joints**: Intermediate nodes (child count apparent from visual relationships)
- **Buckets**: Leaf nodes with item count display
- **Active/Inactive**: Visual distinction using `active` property
- **Weights**:
  - Display weight values where applicable
  - Influence parent-child relationship visualization (e.g., line thickness, spacing)

### 2. Node Information on Hover
- Path display (e.g., ".0.1.2")
- Node type (Joint/Bucket) with relevant counts
- Order type (InOrder/Random/Shuffle)
- Weight information if set
- Active status
- Additional context as available

### 3. Placeholder Editing Interface

#### Read-Only Focus with UI Placeholders
- **Add Operations**: Placeholder buttons for adding buckets/joints
- **Delete Operations**: Placeholder delete buttons (with empty node validation)
- **Edit Operations**:
  - Weight modification (numeric input placeholders)
  - Order type selection (dropdown placeholders)
  - Filter editing (string input placeholders - not connected to TableView yet)
  - Bucket contents (placeholder items "Item 1", "Item 2", etc.)

#### No Drop Zones Initially
- Focus on button-based interactions
- Save drag-and-drop for future enhancement

### 4. Integration Considerations

#### Player UI Integration (Stretch Goal)
- Album art (small)
- Play/pause, next, previous buttons
- Collapsible recently played items list
- Generic playback placeholder for now
- Consider how to "put spigot visuals in a box" with player UI

### 5. Technical Implementation

#### Rendering Implementation Status
- ❌ **Canvas-based rendering**: Moved to trash (equivalent complexity to SVG)
- ✅ **SVG-based rendering**: Implemented with direct parent-child connections and root convergence point
- ❌ **HTML/CSS-based rendering**: Ruled out - cannot handle diagonal connections elegantly with pure HTML/CSS

#### Framework Constraints
- Keep VanJS and existing build system
- Minimize dependencies
- Throw away existing app.ts content

#### Data Assumptions
- Filters: strings (placeholder UI only)
- Bucket items: strings (use count from TableView, generate placeholder content)
- No real TableView filter data yet (future enhancement)

## Success Criteria
- [x] Clear visual hierarchy showing network structure with depth-based columns
- [x] Proper tree layout with buckets distributed vertically by position
- [x] Responsive hover information display with node details
- [x] Placeholder editing UI ready **for future functionality**
- [x] SVG rendering with direct parent-child connections (ruled out Canvas/HTML alternatives)
- [x] Root convergence point visualization for implied spigot root
- [x] Integration-ready design for player UI addition (**placeholder** implemented)

## Implementation Notes

### Layout Understanding
The TableView data structure represents a tree laid out in a specific format:
- **rows[]**: Each row represents a depth level in the tree (left to right progression)
- **position**: Vertical position within that depth level
- **display_width**: How much vertical space the node should occupy
- **parent_position**: Links to parent's position in previous row

### Key Layout Algorithm
```
for each row (depth level):
  x = row_index * CELL_X_STRIDE  // horizontal position
  y = 0
  for each cell in row:
    if cell has node:
      cellY = y * CELL_Y_STRIDE + (display_width * CELL_Y_STRIDE) / 2
      render node at (x, cellY) with height = display_width * CELL_Y_STRIDE
    y += cell.display_width
```

### Connection Strategy
- Root convergence point at fixed position for all top-level nodes (path depth 1)
- Direct lines from parent to child nodes (no right angles)
- Parent position lookup via path string manipulation

## Future Enhancements
- Real filter system integration
- Actual bucket content display
- Drag-and-drop interactions
- Undo/redo functionality
- Full CRUD operations via ModifyCmd system
