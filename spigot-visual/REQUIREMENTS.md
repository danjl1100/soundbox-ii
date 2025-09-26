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
- **Bucket-Centered**: Buckets equally spaced as rows
- **Depth-Based Indentation**: Different indentation levels by hierarchy depth
- **Joint Positioning**: Joints vertically centered around their child nodes
- **Use TableView Data**: Leverage existing `display_width`, `position`, `parent_position` calculations

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

#### Multiple Prototype Versions
- Canvas-based rendering (performance)
- SVG-based rendering (precision/accessibility)
- HTML/CSS-based rendering (simplicity)

#### Framework Constraints
- Keep VanJS and existing build system
- Minimize dependencies
- Throw away existing app.ts content

#### Data Assumptions
- Filters: strings (placeholder UI only)
- Bucket items: strings (use count from TableView, generate placeholder content)
- No real TableView filter data yet (future enhancement)

## Success Criteria
- Clear visual hierarchy showing network structure
- Intuitive bucket-centered layout with proper joint positioning
- Responsive hover information display
- Placeholder editing UI ready for future functionality
- Multiple rendering prototypes to compare approaches
- Integration-ready design for player UI addition

## Future Enhancements
- Real filter system integration
- Actual bucket content display
- Drag-and-drop interactions
- Undo/redo functionality
- Full CRUD operations via ModifyCmd system
