/**
 * TypeScript types for SQL Server Execution Plan (ShowPlan XML)
 * Based on http://schemas.microsoft.com/sqlserver/2004/07/showplan
 */

// Root structure
export interface ShowPlanXML {
  version: string;
  build: string;
  batches: Batch[];
}

export interface Batch {
  statements: Statement[];
}

export interface Statement {
  statementId: number;
  statementText: string;
  statementType: string;
  statementSubTreeCost: number;
  statementEstRows: number;
  statementOptmLevel?: string;
  queryHash?: string;
  queryPlanHash?: string;
  queryPlan: QueryPlan;
}

export interface QueryPlan {
  degreeOfParallelism: number;
  cachedPlanSize?: number;
  compileTime?: number;
  compileCPU?: number;
  compileMemory?: number;
  memoryGrant?: MemoryGrantInfo;
  relOp: RelOp;
  parameters?: Parameter[];
}

export interface MemoryGrantInfo {
  serialRequiredMemory: number;
  serialDesiredMemory: number;
  grantedMemory: number;
  maxUsedMemory: number;
}

export interface Parameter {
  column: string;
  dataType: string;
  compiledValue?: string;
  runtimeValue?: string;
}

// RelOp - The main operator node
export interface RelOp {
  nodeId: number;
  physicalOp: PhysicalOperator;
  logicalOp: string;
  estimateRows: number;
  estimatedRowsRead?: number;
  estimateCPU: number;
  estimateIO: number;
  estimatedTotalSubtreeCost: number;
  avgRowSize: number;
  parallel: boolean;
  estimatedExecutionMode?: 'Row' | 'Batch';
  estimateRebinds?: number;
  estimateRewinds?: number;
  tableCardinality?: number;
  
  // Generic key-value pair for any attribute found in the XML
  attributes?: Record<string, string>;

  // Output columns
  outputColumns: ColumnReference[];
  
  // Runtime information (actual execution stats)
  runtimeInfo?: RuntimeInfo;
  
  // Child operators
  children: RelOp[];
  
  // Operation-specific details
  operationDetails: OperationDetails;
}

export interface RuntimeInfo {
  threadId: number;
  actualRows: number;
  actualRowsRead?: number;
  actualExecutions: number;
  
  // Generic attributes
  attributes?: Record<string, string>;
  
  actualEndOfScans?: number;
  actualElapsedMs: number;
  actualCPUMs: number;
  actualScans?: number;
  actualLogicalReads?: number;
  actualPhysicalReads?: number;
  actualReadAheads?: number;
  actualLobLogicalReads?: number;
  actualLobPhysicalReads?: number;
  actualLobReadAheads?: number;
  batches?: number;
  executionMode?: 'Row' | 'Batch';
}

export interface ColumnReference {
  database?: string;
  schema?: string;
  table?: string;
  alias?: string;
  column: string;
}

export interface OperationDetails {
  // For Index operations
  indexScan?: IndexScanDetails;
  
  // For Nested Loops
  nestedLoops?: NestedLoopsDetails;
  
  // For Hash operations
  hash?: HashDetails;
  
  // For Merge Join
  merge?: MergeDetails;
  
  // For Sort
  sort?: SortDetails;
  
  // For Compute Scalar
  computeScalar?: ComputeScalarDetails;
  
  // For Filter
  filter?: FilterDetails;
  
  // For Parallelism
  parallelism?: ParallelismDetails;
  
  // For Stream Aggregate
  streamAggregate?: AggregateDetails;
  
  // For Hash Aggregate
  hashAggregate?: AggregateDetails;
  
  // Raw attributes for any other details
  raw?: Record<string, unknown>;
}

export interface IndexScanDetails {
  ordered: boolean;
  scanDirection?: 'FORWARD' | 'BACKWARD';
  forcedIndex: boolean;
  forceSeek: boolean;
  forceScan: boolean;
  noExpandHint: boolean;
  storage: 'RowStore' | 'ColumnStore';
  object: ObjectReference;
  seekPredicates?: SeekPredicate[];
  predicate?: string;
  definedValues?: DefinedValue[];
}

export interface ObjectReference {
  database?: string;
  schema?: string;
  table: string;
  index?: string;
  indexKind?: 'Clustered' | 'NonClustered' | 'Heap' | 'Columnstore';
  storage?: string;
  alias?: string;
}

export interface SeekPredicate {
  prefix?: {
    scanType: string;
    rangeColumns: ColumnReference[];
    rangeExpressions: string[];
  };
}

export interface DefinedValue {
  column: ColumnReference;
  expression?: string;
}

export interface NestedLoopsDetails {
  optimized: boolean;
  outerReferences?: ColumnReference[];
  predicate?: string;
}

export interface HashDetails {
  buildResidual?: string;
  probeResidual?: string;
  hashKeysBuild?: ColumnReference[];
  hashKeysProbe?: ColumnReference[];
}

export interface MergeDetails {
  manyToMany: boolean;
  innerSideJoinColumns?: ColumnReference[];
  outerSideJoinColumns?: ColumnReference[];
  residual?: string;
}

export interface SortDetails {
  distinct: boolean;
  orderBy: OrderByColumn[];
}

export interface OrderByColumn {
  column: ColumnReference;
  ascending: boolean;
}

export interface ComputeScalarDetails {
  definedValues: DefinedValue[];
}

export interface FilterDetails {
  predicate: string;
  startupExpression: boolean;
}

export interface ParallelismDetails {
  type: 'Distribute Streams' | 'Gather Streams' | 'Repartition Streams';
  partitionColumns?: ColumnReference[];
  orderBy?: OrderByColumn[];
}

export interface AggregateDetails {
  groupBy?: ColumnReference[];
  definedValues: DefinedValue[];
}

// Physical operator types
export type PhysicalOperator = 
  // Scans and Seeks
  | 'Clustered Index Scan'
  | 'Clustered Index Seek'
  | 'Nonclustered Index Scan'
  | 'Nonclustered Index Seek'
  | 'Index Scan'
  | 'Index Seek'
  | 'Table Scan'
  | 'RID Lookup'
  | 'Key Lookup'
  | 'Columnstore Index Scan'
  | 'Columnstore Index Seek'
  
  // Joins
  | 'Nested Loops'
  | 'Hash Match'
  | 'Merge Join'
  | 'Adaptive Join'
  
  // Aggregation
  | 'Stream Aggregate'
  | 'Hash Aggregate'
  | 'Window Aggregate'
  
  // Sorting and Filtering
  | 'Sort'
  | 'Top'
  | 'Filter'
  | 'Top Sort'
  
  // Data Modification
  | 'Insert'
  | 'Update'
  | 'Delete'
  | 'Merge'
  | 'Table Insert'
  | 'Index Insert'
  | 'Clustered Index Insert'
  | 'Table Update'
  | 'Index Update'
  | 'Clustered Index Update'
  | 'Table Delete'
  | 'Index Delete'
  | 'Clustered Index Delete'
  
  // Parallelism
  | 'Parallelism'
  | 'Distribute Streams'
  | 'Gather Streams'
  | 'Repartition Streams'
  
  // Spools
  | 'Table Spool'
  | 'Index Spool'
  | 'Row Count Spool'
  | 'Window Spool'
  | 'Eager Spool'
  | 'Lazy Spool'
  
  // Miscellaneous
  | 'Compute Scalar'
  | 'Concatenation'
  | 'Constant Scan'
  | 'Sequence'
  | 'Sequence Project'
  | 'Segment'
  | 'Assert'
  | 'Bitmap'
  | 'Parameter Table Scan'
  | 'Split'
  | 'Collapse'
  | 'UDX'
  | 'Remote Query'
  | 'Remote Scan'
  | 'Remote Insert'
  | 'Remote Update'
  | 'Remote Delete'
  | string; // Allow other operators not in list

// Icon mapping for operators
export const operatorIcons: Record<string, string> = {
  // Scans - database icon
  'Clustered Index Scan': 'fa-database',
  'Nonclustered Index Scan': 'fa-database',
  'Index Scan': 'fa-database',
  'Table Scan': 'fa-table',
  'Columnstore Index Scan': 'fa-bars',
  
  // Seeks - magnifying glass
  'Clustered Index Seek': 'fa-magnifying-glass',
  'Nonclustered Index Seek': 'fa-magnifying-glass',
  'Index Seek': 'fa-magnifying-glass',
  'Columnstore Index Seek': 'fa-magnifying-glass',
  
  // Lookups - key
  'Key Lookup': 'fa-key',
  'RID Lookup': 'fa-key',
  
  // Joins - link
  'Nested Loops': 'fa-link',
  'Hash Match': 'fa-hashtag',
  'Merge Join': 'fa-code-merge',
  'Adaptive Join': 'fa-code-branch',
  
  // Aggregation - calculator
  'Stream Aggregate': 'fa-calculator',
  'Hash Aggregate': 'fa-calculator',
  'Window Aggregate': 'fa-window-maximize',
  
  // Sorting - sort icons
  'Sort': 'fa-arrow-down-a-z',
  'Top Sort': 'fa-arrow-up-1-9',
  'Top': 'fa-arrow-up',
  
  // Filter
  'Filter': 'fa-filter',
  
  // Parallelism - network
  'Parallelism': 'fa-network-wired',
  'Distribute Streams': 'fa-arrows-split-up-and-left',
  'Gather Streams': 'fa-compress',
  'Repartition Streams': 'fa-shuffle',
  
  // Spools - memory
  'Table Spool': 'fa-memory',
  'Index Spool': 'fa-memory',
  'Row Count Spool': 'fa-memory',
  'Eager Spool': 'fa-memory',
  'Lazy Spool': 'fa-memory',
  
  // DML
  'Insert': 'fa-plus',
  'Update': 'fa-pen',
  'Delete': 'fa-trash',
  'Merge': 'fa-code-merge',
  'Table Insert': 'fa-plus',
  'Clustered Index Insert': 'fa-plus',
  'Table Update': 'fa-pen',
  'Clustered Index Update': 'fa-pen',
  'Table Delete': 'fa-trash',
  'Clustered Index Delete': 'fa-trash',
  
  // Misc
  'Compute Scalar': 'fa-square-root-variable',
  'Concatenation': 'fa-object-group',
  'Constant Scan': 'fa-circle-dot',
  'Sequence': 'fa-list-ol',
  'Assert': 'fa-circle-exclamation',
  'Bitmap': 'fa-grip',
  'Segment': 'fa-section',
  
  // Default
  'default': 'fa-circle-nodes'
};

// Get icon class for an operator
export function getOperatorIcon(operator: string): string {
  return operatorIcons[operator] || operatorIcons['default'];
}

// Cost severity levels
export type CostSeverity = 'low' | 'medium' | 'high' | 'critical';

export function getCostSeverity(costPercentage: number): CostSeverity {
  if (costPercentage < 10) return 'low';
  if (costPercentage < 30) return 'medium';
  if (costPercentage < 50) return 'high';
  return 'critical';
}

export function getCostColor(severity: CostSeverity): string {
  switch (severity) {
    case 'low': return import.meta.env.VITE_COLOR_COST_LOW || '#22c55e';
    case 'medium': return import.meta.env.VITE_COLOR_COST_MEDIUM || '#eab308';
    case 'high': return import.meta.env.VITE_COLOR_COST_HIGH || '#f97316';
    case 'critical': return import.meta.env.VITE_COLOR_COST_CRITICAL || '#ef4444';
  }
}

// Utility to format time
export function formatTime(ms: number): string {
  if (ms < 1) return '<1ms';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

// Utility to format rows
export function formatRows(rows: number): string {
  if (rows < 1000) return rows.toString();
  if (rows < 1000000) return `${(rows / 1000).toFixed(1)}K`;
  return `${(rows / 1000000).toFixed(1)}M`;
}

// Utility to format cost percentage
export function formatCostPercentage(cost: number, totalCost: number): string {
  if (totalCost === 0) return '0%';
  return `${((cost / totalCost) * 100).toFixed(1)}%`;
}
