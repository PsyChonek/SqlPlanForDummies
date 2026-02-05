# SQL Plan For Dummies

An interactive SQL Execution Plan Viewer built with Tauri + Vue + D3.js that provides visual analysis and performance insights for query execution plans.

## Features

### 🌊 Waveform Visualization
- Interactive circular graph layout using physics-based positioning
- Color-coded nodes based on temporal cost and density factors
- Animated connections showing data flow between operations
- Click nodes to inspect detailed metrics

### 📊 Diagnostic Console
- Real-time telemetry readings for selected operations
- Health score calculation based on multiple factors
- Efficiency grading system (STELLAR, ROBUST, ADEQUATE, SUBOPTIMAL)
- Detailed annotation display for operation metadata

### 📡 Artifact Ingestion Terminal
- Drag-and-drop JSON file upload
- Automatic schema transformation
- Built-in prototype data for testing
- Support for various JSON execution plan formats

### 🔬 Intelligence Hub
- Automated diagnostic analysis
- Performance concern detection (critical/warning/advisory levels)
- Severity visualization with impact metrics
- Knowledge repository with optimization guidance

## Getting Started

### Prerequisites
- Node.js (v16 or higher)
- npm or yarn
- Rust (for Tauri development)

### Installation

```bash
# Install dependencies
npm install

# Run in development mode
npm run dev

# Build for production
npm run build

# Run Tauri app
npm run tauri dev
```

## Usage

1. **Load a Plan**: Use the Artifact Ingestion Terminal to:
   - Drag and drop a JSON execution plan file
   - Or click "Materialize Prototype Schema" to load sample data

2. **Explore the Visualization**: 
   - Click on nodes in the waveform graph to inspect details
   - Nodes are colored by performance characteristics
   - Connections show parent-child relationships

3. **Analyze Performance**:
   - View detailed metrics in the Diagnostic Console
   - Check the Intelligence Hub for performance warnings
   - Review optimization recommendations

## Supported JSON Format

The application accepts execution plans in various JSON formats. Example:

```json
{
  "id": "node_1",
  "operator": "HASH_JOIN",
  "rows": 10000,
  "time": 250,
  "cost": 0.75,
  "children": [
    {
      "id": "node_2",
      "operator": "TABLE_SCAN",
      "rows": 50000,
      "time": 120
    }
  ]
}
```

The system automatically maps common field names:
- `id/nodeId/identifier` → keystone
- `operator/operation/nodeType` → nomenclature
- `rows/estimatedRows/rowCount` → magnitude
- `time/duration/actualTime` → temporalCost
- `cost/weight/priority` → densityFactor
- `children/inputs/subNodes` → offspring

## Architecture

- **Frontend**: Vue 3 with Composition API
- **Visualization**: D3.js for SVG rendering
- **Desktop App**: Tauri 2.0
- **State Management**: Custom composable-based state
- **Styling**: Scoped CSS with gradients and animations

## Components

- `WaveformRenderer.vue` - Main visualization component with circular layout
- `DiagnosticConsole.vue` - Detailed node inspection panel
- `ArtifactTerminal.vue` - File upload and data ingestion
- `IntelligenceHub.vue` - Performance analysis and recommendations
- `planState.ts` - Centralized state management composable

## License

See LICENSE file

## Contributing

This is an educational project demonstrating SQL execution plan visualization techniques.
