# AGENTS.md - AI Development Guidelines for SqlPlanForDummies

## Project Overview

SqlPlanForDummies is an interactive SQL Server Execution Plan Viewer built with Tauri + Vue 3 + D3.js. It provides visual analysis and performance insights for query execution plans stored in `.sqlplan` XML format.

## Tech Stack

| Component | Technology | Purpose |
|-----------|------------|---------|
| Desktop Framework | Tauri 2.x | Native desktop app shell |
| Frontend | Vue 3 (Composition API) | UI framework |
| Visualization | D3.js v7 | Graph rendering and node layout |
| Build Tool | Vite 6.x | Development and bundling |
| Language | TypeScript | Type safety |
| CSS Framework | Tailwind CSS | Utility-first CSS framework |
| Icons | FontAwesome 6 | Icon library (no emojis) |
| Styling | SCSS | CSS preprocessing |

## File Structure

```
src/
  App.vue                     # Main application shell
  main.ts                     # Entry point
  styles/
    main.css                  # Tailwind CSS imports and custom styles
  components/
    ExecutionPlanGraph.vue    # D3.js tree layout visualization  
    NodeDetails.vue           # Selected node details panel
    PlanLoader.vue            # File loading interface
    AnalysisPanel.vue         # Performance analysis and issues
  composables/
    planState.ts              # Global state management
    sqlPlanParser.ts          # XML parsing for .sqlplan files
  types/
    sqlplan.ts                # TypeScript types for SQL Plan structure
examples/
  company.sqlplan             # Example execution plan
public/
  examples/
    company.sqlplan           # Served example plan
```

## Coding Standards

### Vue Components

- Use `<script setup lang="ts">` for all components
- Props and emits must be typed
- Use composables for shared logic
- Prefer `ref()` and `computed()` over reactive objects for primitives

### TypeScript

- Strict mode enabled
- No `any` types - define proper interfaces
- Use barrel exports from `types/` folder

### CSS/Styling

- Use Tailwind CSS utility classes for all styling
- Follow dark theme with slate color palette
- No inline styles except for dynamic D3 calculations
- FontAwesome icons only - NO emojis in UI

### State Management

- Use `composables/planState.ts` for global state
- Avoid prop drilling - use provide/inject for deep components

## SQLPlan XML Schema

The `.sqlplan` file format is SQL Server ShowPlan XML. Key elements:

```xml
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple StatementText="..." StatementSubTreeCost="...">
          <QueryPlan>
            <RelOp NodeId="0" PhysicalOp="..." LogicalOp="..." EstimateRows="...">
              <!-- Nested RelOp elements form the execution tree -->
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>
```

### Key Attributes to Extract

| Attribute | Description |
|-----------|-------------|
| `NodeId` | Unique identifier for each operation |
| `PhysicalOp` | Physical operator type (e.g., Clustered Index Seek) |
| `LogicalOp` | Logical operation (e.g., Inner Join) |
| `EstimateRows` | Estimated row count |
| `EstimateCPU` | CPU cost estimate |
| `EstimateIO` | I/O cost estimate |
| `EstimatedTotalSubtreeCost` | Total cost including children |
| `ActualRows` | Actual rows (from RunTimeInformation) |
| `ActualExecutions` | Number of executions |
| `ActualElapsedms` | Actual elapsed time in ms |
| `ActualCPUms` | Actual CPU time in ms |
| `ActualLogicalReads` | Logical reads performed |
| `ActualPhysicalReads` | Physical reads performed |

## Graph Visualization Requirements

### Node Representation

Each `RelOp` element becomes a graph node with:
- Icon based on PhysicalOp type (FontAwesome)
- Color coding based on cost percentage
- Size based on row count
- Tooltip with key metrics

### Edge Representation

Connections between nodes show:
- Data flow direction (parent to child)
- Line thickness based on row count
- Arrow direction indicating data flow

### Layout Algorithm

- Tree layout (top-to-bottom or left-to-right)
- Parent nodes at top/left, children below/right
- Zoom and pan support
- Click to select and show details

## Performance Indicators

### Cost Thresholds

| Range | Severity | Color |
|-------|----------|-------|
| 0-10% | Low | Green (#48c774) |
| 10-30% | Medium | Yellow (#ffdd57) |
| 30-50% | High | Orange (#ff9f43) |
| 50%+ | Critical | Red (#f14668) |

### Warning Triggers

- Table scans on large tables
- Missing index warnings
- Implicit conversions
- Key lookups with high row counts
- Parallelism issues
- Memory grants exceeding thresholds

## AI Agent Instructions

### When Implementing Features

1. Always parse XML correctly using DOMParser
2. Handle multiple statements in a single plan
3. Preserve all `RelOp` attributes for detailed view
4. Use FontAwesome icons - NEVER emojis
5. Apply Tailwind CSS utility classes for consistent styling
6. Test with the example `company.sqlplan` file

### When Fixing Bugs

1. Check browser console for errors
2. Verify XML namespace handling
3. Test with different plan complexities
4. Ensure TypeScript types match runtime data

### When Adding UI Components

1. Follow existing component patterns
2. Use Tailwind CSS utility classes for styling
3. Add FontAwesome icons with `<i class="fa-solid fa-*"></i>`
4. Maintain dark theme consistency (bg-slate-*, text-slate-*)

## Feature Roadmap

### Phase 1: Core Parsing (P0) ✓
- [x] XML parser for .sqlplan files
- [x] Type definitions for SQL Plan structure
- [x] Extract RelOp tree with all attributes

### Phase 2: Visualization (P0) ✓
- [x] Tree layout for execution plan graph (left-to-right)
- [x] Node icons based on operator type
- [x] Cost-based color coding (green/yellow/orange/red)
- [x] Interactive node selection
- [x] Zoom controls (+, -, fit)
- [x] Keyboard navigation (arrow keys)
- [x] Export to SVG/PNG

### Phase 3: Details Panel (P1) ✓
- [x] Comprehensive node metrics display
- [x] Estimated vs Actual comparisons
- [x] Index and table information
- [x] Predicate display (seek/scan conditions)
- [x] Output columns list

### Phase 4: Analysis (P2) ✓
- [x] Performance issue detection engine
- [x] Table scan warnings
- [x] Key lookup warnings
- [x] Plan comparison view
- [x] Operator difference highlighting

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Arrow Left | Navigate to parent node |
| Arrow Right | Navigate to first child |
| Arrow Up | Navigate to previous sibling |
| Arrow Down | Navigate to next sibling |
| + or = | Zoom in |
| - | Zoom out |
| 0 | Fit to view |
| Escape | Deselect node |

## File Structure Update

```
src/
  components/
    ExecutionPlanGraph.vue    # D3.js tree with zoom/export/keyboard nav
    NodeDetails.vue           # Node details with predicates
    PlanLoader.vue            # File loading with drag/drop
    AnalysisPanel.vue         # Issue detection
    PlanComparison.vue        # Side-by-side plan comparison
```

## Commands

```bash
# Development
npm install
npm run dev

# Build
npm run build

# Tauri Development
npm run tauri dev

# Tauri Build
npm run tauri build
```

## Dependencies Added

```json
{
  "dependencies": {
    "tailwindcss": "^4.0.0",
    "@tailwindcss/vite": "^4.0.0",
    "@fortawesome/fontawesome-free": "^6.5.0"
  }
}
```

## Import Setup

```typescript
// main.ts
import './styles/main.css';

// styles/main.css
@import "tailwindcss";
@import "@fortawesome/fontawesome-free/css/all.min.css";
```

## Rules

- Never use emojis in the UI - only FontAwesome icons
- Always maintain a consistent dark theme with slate colors