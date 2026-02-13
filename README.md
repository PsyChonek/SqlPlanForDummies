# SQL Plan For Dummies

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-blue.svg)](https://github.com/yourusername/SqlPlanForDummies)

An interactive SQL Execution Plan Viewer built with Tauri + Vue + D3.js that provides visual analysis and performance insights for query execution plans.

![SQL Plan For Dummies](docs/main-page.png)
*Visual execution plan analysis with performance insights*

## Quick Start

1. **Install**: `winget install SqlPlanForDummies`
2. **Get a plan**: In SSMS, enable "Include Actual Execution Plan" (Ctrl+M), run your query, and save as `.sqlplan`
3. **Analyze**: Drag the `.sqlplan` file into SQL Plan For Dummies
4. **Optimize**: Review the automated analysis tab for performance recommendations

[Jump to Installation](#installation) | [Jump to Usage Guide](#usage) | [View Roadmap](#roadmap)

## Features

### Core Visualization

- **Interactive Tree Layout**: D3.js powered hierarchical visualization of execution plan operators
- **Performance Indicators**: Color-coded nodes (Green/Yellow/Orange/Red) based on relative cost and severity
- **Visual Cues**:
  - Operator-specific icons (Index Seek, Table Scan, Nested Loops, Hash Join, etc.)
  - Dynamic line thickness representing row count and data flow volume
  - Animated transitions for smooth navigation
- **Navigation Controls**:
  - Mouse: Click to select, drag to pan, scroll to zoom
  - Keyboard: Arrow keys for node traversal, +/- for zoom
  - Minimap overview for large plans

### Detailed Analysis

- **Comprehensive Node Inspection**:
  - CPU time, I/O cost, memory grants
  - Estimated vs actual row counts
  - Execution statistics and metrics
  - Operator-specific properties
- **Estimates vs Actuals**: Visual indicators when optimizer estimates diverge significantly from actual runtime values
- **Predicate Analysis**: Formatted and readable display of:
  - Seek Predicates (index key lookups)
  - Scan Predicates (filter conditions)
  - Join conditions and clauses
- **Output Column Lists**: Track which columns flow through each operator
- **Object Information**: Table names, index names, schema details

### Automated Insights

- **Performance Warning Detection**:
  - Table/Index scans on large tables
  - Missing index recommendations
  - Implicit conversions affecting performance
  - High-cost sort and hash operations
  - Key lookups and bookmark lookups
  - Spills to tempdb
- **Impact Assessment**: Automatic identification of bottleneck operators
- **Plan Comparison Engine**:
  - Side-by-side visual comparison
  - Difference highlighting (added/removed/modified operators)
  - Cost delta analysis

### File Support & Export

- **.sqlplan Parsing**: Full support for SQL Server XML ShowPlan format (SSMS and Azure Data Studio compatible)
- **Drag & Drop**: Simply drag .sqlplan files into the app
- **Export Options**:
  - PNG export for documentation and reports
  - SVG export for scalable graphics
  - Plan data export (JSON)
- **Example Plans**: Built-in sample plans for exploration

### Desktop Application Features

- **Native Performance**: Built with Tauri for fast, lightweight desktop experience
- **Windows Native**: Optimized for Windows 10 and Windows 11
- **Offline Capable**: No internet connection required
- **File Associations**: Register to open .sqlplan files directly from Windows Explorer
- **Auto-updates**: Seamless updates to the latest version

## Roadmap

### Planned Features

- [ ] **Query Text Highlighting**: Syntax highlighting for T-SQL statement text
- [ ] **History & Sessions**: Recently opened plans with quick access
- [ ] **Multi-Tab Interface**: Open and compare multiple plans simultaneously
- [ ] **Advanced Search**: Filter nodes by operator type, table name, or cost threshold
- [ ] **Dark/Light Mode**: Theme toggle (currently dark mode only)
- [ ] **SSMS Integration**: Right-click menu in SSMS to open plans directly
- [ ] **Azure Data Studio Extension**: Native extension for seamless integration
- [ ] **Index Advisor**: AI-powered missing index recommendations with CREATE INDEX scripts
- [ ] **Performance Baseline**: Save and compare against baseline execution plans
- [ ] **Export to Excel**: Detailed cost breakdown in Excel format
- [ ] **Command Line Interface**: `sqlplan-cli analyze query.sqlplan` for CI/CD integration
- [ ] **Batch Analysis**: Compare multiple plans at once to identify regressions

### Under Consideration

- [ ] **PostgreSQL Support**: EXPLAIN ANALYZE format parsing
- [ ] **MySQL Support**: EXPLAIN format parsing
- [ ] **Custom Themes**: User-defined color schemes
- [ ] **Telemetry Dashboard**: Aggregate performance insights across multiple plans

## Installation

### Option 1: Install Pre-built Binary (Recommended)

#### Windows (Winget)

```bash
winget install SqlPlanForDummies
```

#### Windows (MSI Installer)

Download the latest `.msi` installer from [Releases](https://github.com/yourusername/SqlPlanForDummies/releases) and run it.

### Option 2: Build from Source

#### Prerequisites

- **Node.js** (v18 or higher) - [Download](https://nodejs.org/)
- **npm** or **yarn** or **pnpm**
- **Rust** (for Tauri) - [Install Rust](https://rustup.rs/)
- **System Dependencies**:
  - **Windows**: WebView2 (usually pre-installed on Windows 10+)

#### Development Setup

1. **Clone the repository**

   ```bash
   git clone https://github.com/yourusername/SqlPlanForDummies.git
   cd SqlPlanForDummies
   ```

1. **Install dependencies**

   ```bash
   npm install
   ```

1. **Run in development mode**

   **Web-only (Vue + Vite hot reload):**

   ```bash
   npm run dev
   # Opens http://localhost:5173 in your browser
   ```

   **Desktop app (Tauri):**

   ```bash
   npm run tauri dev
   # Launches the Tauri desktop application with hot reload
   # Changes to Vue components auto-refresh
   # Changes to Rust code trigger rebuild
   ```

1. **Build for production**

   **Web build:**

   ```bash
   npm run build
   # Outputs to dist/
   ```

   **Desktop app build:**

   ```bash
   npm run tauri build
   # Creates Windows installers in src-tauri/target/release/bundle/
   # Output: .msi installer and .exe portable
   ```

#### Development Tips

- **Hot Reload**: Both `npm run dev` and `npm run tauri dev` support hot module replacement
- **Debugging**:
  - Open DevTools in Tauri app: Right-click → Inspect Element
  - Rust logs: Check terminal output during `npm run tauri dev`
  - Vue DevTools: Install the browser extension
- **Linting**:

  ```bash
  npm run lint          # Check for issues
  npm run lint:fix      # Auto-fix issues
  ```

- **Type Checking**:

  ```bash
  npm run type-check    # TypeScript validation
  ```

## Usage

### Getting an Execution Plan from SQL Server

Before you can use SQL Plan For Dummies, you need to capture an execution plan from SQL Server:

**SQL Server Management Studio (SSMS):**

1. Run your query with "Include Actual Execution Plan" enabled (Ctrl+M)
2. After execution, right-click the execution plan tab
3. Select "Save Execution Plan As..." and save as `.sqlplan`

**Azure Data Studio:**

1. Enable "Explain" or "Actual Plan" before running your query
2. Right-click the plan and export as `.sqlplan`

**T-SQL (for actual plan):**

```sql
SET STATISTICS XML ON;
-- Your query here
SELECT * FROM YourTable WHERE ...;
SET STATISTICS XML OFF;
-- Copy the XML output to a .sqlplan file
```

### Loading and Analyzing Plans

1. **Load a Plan**:
   - **Drag & Drop**: Simply drag a `.sqlplan` file onto the application window
   - **File Browser**: Click "Open File" or "Load Plan" button to browse
   - **Example Plans**: Click "Load Example" to explore a sample plan

2. **Navigate the Visualization**:
   - **Pan**: Click and drag the background to move around
   - **Zoom**: Use mouse scroll wheel or trackpad pinch gesture
   - **Select Node**: Click any operator node to view details
   - **Keyboard Navigation**: Use Arrow keys to move between nodes
   - **Reset View**: Click the "Reset Zoom" button to fit the entire plan

3. **Inspect Node Details**:
   - Click any node to open the **Details Panel** on the right
   - Review cost metrics, row counts, and execution time
   - Check for estimate vs actual discrepancies (highlighted in red/orange)
   - Examine predicates, output lists, and properties

4. **Review Analysis**:
   - Switch to the **Analysis** tab to see automated performance warnings
   - Issues are categorized by severity (Critical, Warning, Info)
   - Each warning includes a description and potential impact

5. **Compare Plans** (if multiple plans loaded):
   - Open the **Comparison** view
   - Load a second plan to compare side-by-side
   - Review cost differences and structural changes

6. **Export**:
   - Click "Export as PNG" or "Export as SVG" to save the visualization
   - Share with team members or include in documentation

## Supported Formats

### SQL Server Execution Plans

The application supports **SQL Server XML ShowPlan** format (`.sqlplan` files).

**Supported SQL Server Versions:**

- SQL Server 2022
- SQL Server 2019
- SQL Server 2017
- SQL Server 2016
- SQL Server 2014
- SQL Server 2012 and earlier (with limited feature support)
- Azure SQL Database
- Azure SQL Managed Instance

**Source Tools:**

- SQL Server Management Studio (SSMS) - All versions
- Azure Data Studio
- SQL Server Profiler (saved as .sqlplan)
- T-SQL (SET STATISTICS XML ON)

**Example Format:**

```xml
<ShowPlanXML xmlns="http://schemas.microsoft.com/sqlserver/2004/07/showplan" Version="1.6">
  <BatchSequence>
    <Batch>
      <Statements>
        <StmtSimple>
          <QueryPlan>
            <RelOp NodeId="0" PhysicalOp="Clustered Index Scan" ...>
              <!-- Operator details -->
            </RelOp>
          </QueryPlan>
        </StmtSimple>
      </Statements>
    </Batch>
  </BatchSequence>
</ShowPlanXML>
```

## Technology Stack

### Frontend

- **Framework**: Vue 3 with Composition API and `<script setup>` syntax
- **Build Tool**: Vite 5 for fast development and optimized production builds
- **Language**: TypeScript for type safety
- **Visualization**: D3.js v7 for SVG-based tree rendering and animations
- **Styling**: Scoped CSS with CSS custom properties (variables)

### Desktop Runtime

- **Tauri 2.0**: Rust-based desktop framework
- **Webview**: Microsoft Edge WebView2 (Chromium-based)
- **Backend**: Rust for native file system access and performance

### State Management

- **Composables**: Vue 3 Composition API with reactive state
- **No external store library**: Lightweight custom state management

### Key Libraries

- **XML Parsing**: Native browser DOMParser for .sqlplan files
- **Export**: html2canvas for PNG export, native SVG serialization

## Project Structure

```text
SqlPlanForDummies/
├── src/
│   ├── components/
│   │   ├── ExecutionPlanGraph.vue    # Main D3.js visualization
│   │   ├── NodeDetails.vue           # Node inspection panel
│   │   ├── PlanLoader.vue            # File drag-drop and loading
│   │   ├── AnalysisPanel.vue         # Performance warnings
│   │   └── PlanComparison.vue        # Side-by-side comparison
│   ├── composables/
│   │   └── planState.ts              # Centralized state management
│   ├── types/
│   │   └── plan.ts                   # TypeScript interfaces
│   ├── utils/
│   │   ├── planParser.ts             # XML parsing logic
│   │   └── analysis.ts               # Performance analysis rules
│   ├── App.vue                       # Root component
│   └── main.ts                       # Application entry point
├── src-tauri/
│   ├── src/
│   │   └── main.rs                   # Tauri backend (Rust)
│   ├── Cargo.toml                    # Rust dependencies
│   └── tauri.conf.json               # Tauri configuration
├── public/                           # Static assets
└── package.json                      # Node dependencies and scripts
```

## FAQ

### General Questions

**Q: Does this work with PostgreSQL or MySQL execution plans?**
A: Currently, only SQL Server `.sqlplan` format is supported. PostgreSQL and MySQL support is on the roadmap.

**Q: Can I use this with estimated execution plans?**
A: Yes! Both estimated and actual execution plans are supported. However, actual plans provide more insights (actual row counts, execution time, etc.).

**Q: Is my data sent anywhere or analyzed online?**
A: No. SQL Plan For Dummies runs entirely offline on your machine. Your execution plans and data never leave your computer.

**Q: Can I open multiple plans at once?**
A: Multi-tab support is on the roadmap. Currently, you can open one plan at a time, but you can use the comparison feature to load a second plan side-by-side.

### Troubleshooting

**Q: The app won't open my `.sqlplan` file**
A: Ensure the file is a valid SQL Server XML ShowPlan format. Try opening it in SSMS first to verify. Some third-party tools may generate incompatible formats.

**Q: I see "WebView2 not found" error on Windows**
A: Install Microsoft Edge WebView2 Runtime from [microsoft.com/edge/webview2](https://developer.microsoft.com/microsoft-edge/webview2/).

**Q: The visualization is too large/small**
A: Use the mouse scroll wheel to zoom in/out, or click the "Reset Zoom" button to fit the entire plan on screen.

**Q: Some nodes are cut off or overlapping**
A: This can happen with very large plans. Try zooming out or using the keyboard arrow keys to navigate. We're working on improved layout algorithms.

## Contributing

Contributions are welcome! This is an open-source educational project demonstrating SQL execution plan visualization techniques.

### How to Contribute

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes** and test thoroughly
4. **Commit your changes**: `git commit -m 'Add some amazing feature'`
5. **Push to the branch**: `git push origin feature/amazing-feature`
6. **Open a Pull Request**

### Development Guidelines

- Follow the existing code style (ESLint + Prettier)
- Add tests for new features (when applicable)
- Update documentation for user-facing changes
- Keep commits focused and atomic

### Areas for Contribution

- Bug fixes and performance improvements
- New analysis rules for performance warnings
- UI/UX enhancements
- Documentation improvements
- Support for additional SQL Server versions
- Accessibility improvements

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- **D3.js** team for the powerful visualization library
- **Tauri** team for the excellent desktop framework
- **Vue.js** community for the reactive framework
- SQL Server community for execution plan insights and best practices

## Support

- **Issues**: [GitHub Issues](https://github.com/yourusername/SqlPlanForDummies/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/SqlPlanForDummies/discussions)
- **Documentation**: [Wiki](https://github.com/yourusername/SqlPlanForDummies/wiki)

---

Made for SQL Server DBAs and Developers
