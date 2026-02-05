# SQL Plan Viewer - Quick Start Guide

## Running the Application

### Development Mode (Browser)
```bash
npm install
npm run dev
```
Then open http://localhost:5173 in your browser

### Desktop App Mode
```bash
npm install
npm run tauri dev
```

## Using the Application

### 1. Load an Execution Plan

**Option A: Use Sample Data**
- Click the "⚡ Materialize Prototype Schema" button
- This loads a sample SQL execution plan for demonstration

**Option B: Load Your Own Plan**
- Drag and drop a JSON file onto the upload area
- Or click the upload area to browse for a file

### 2. Explore the Visualization

The circular graph shows:
- **Nodes**: Each represents an operation in the execution plan
- **Connections**: Show parent-child relationships
- **Colors**: Indicate performance characteristics
  - Orange/Red: High cost operations
  - Blue: Medium cost operations
  - Green: Low cost operations

**Interactions:**
- Click any node to see detailed information
- The graph uses physics-based positioning for optimal layout

### 3. Analyze Performance

**Diagnostic Console (Right Panel):**
- Shows detailed metrics for the selected node
- Displays health score (0-100)
- Efficiency grade (STELLAR, ROBUST, ADEQUATE, SUBOPTIMAL)
- Operation metadata and annotations

**Intelligence Hub (Bottom Panel):**
- Automated diagnostic analysis
- Performance warnings with severity levels:
  - 🔴 Critical: Requires immediate attention
  - ⚠️ Warning: Should be addressed
  - 💡 Advisory: Optimization opportunities
- Knowledge repository with explanations

## JSON Format

The application accepts flexible JSON formats. Here's an example:

```json
{
  "id": "root_node",
  "operator": "HASH_JOIN",
  "rows": 10000,
  "time": 250,
  "cost": 0.75,
  "children": [
    {
      "id": "child_1",
      "operator": "TABLE_SCAN",
      "rows": 50000,
      "time": 120,
      "table": "orders"
    }
  ]
}
```

**Supported Field Names:**
- ID: `id`, `nodeId`, `identifier`
- Operation: `operator`, `operation`, `nodeType`
- Rows: `rows`, `estimatedRows`, `rowCount`
- Time: `time`, `duration`, `actualTime`
- Cost: `cost`, `weight`, `priority`
- Children: `children`, `inputs`, `subNodes`

## Performance Tips

The tool helps identify:
- **Full table scans**: Operations without indexes
- **Expensive joins**: High-cost join operations
- **Memory issues**: Large sort/aggregate operations
- **Cardinality problems**: Poor row estimation
- **Bottlenecks**: Operations with high temporal costs

## Keyboard Shortcuts

- Click nodes to inspect details
- Scroll to zoom in/out on the graph
- Drag to pan the visualization

## Troubleshooting

**JSON won't load:**
- Ensure the file is valid JSON
- Check that it has the required structure (at least an `id` and `operator` field)

**Graph looks cluttered:**
- The physics simulation will settle after a few seconds
- Try clicking individual nodes to focus on specific operations

**Performance issues:**
- For very large plans (100+ nodes), performance may vary
- Consider breaking down complex queries into smaller parts

## Example Workflows

### Workflow 1: Quick Analysis
1. Click "Materialize Prototype Schema"
2. Observe the warning indicators
3. Click on red/orange nodes for details
4. Review suggestions in Intelligence Hub

### Workflow 2: Custom Plan Analysis
1. Export execution plan from your database as JSON
2. Drag-drop the file onto the application
3. Navigate through the graph
4. Identify bottlenecks and optimization opportunities
5. Apply suggested optimizations

## Support

For issues or questions, please check the README.md file or open an issue on the repository.
