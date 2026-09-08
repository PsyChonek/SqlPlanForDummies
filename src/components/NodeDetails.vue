<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { usePlanState } from '../composables/planState';
import CollapsiblePanel from './CollapsiblePanel.vue';
import type { RelOp } from '../types/sqlplan';
import {
  getOperatorIcon,
  getCostSeverity,
  getCostColor,
  formatTime,
  formatRows
} from '../types/sqlplan';

const { state, getNodeCostPercentage, allNodes } = usePlanState();

const selectedNode = computed(() => state.selectedNode);
const selectedEdge = computed(() => state.selectedEdge);

const searchTerm = ref('');

watch(selectedNode, () => { searchTerm.value = ''; });

// Edge metrics for data flow display
const edgeMetrics = computed(() => {
  if (!selectedEdge.value) return [];
  const target = selectedEdge.value.target;
  const runtime = target.runtimeInfo;
  const metrics: { label: string; value: string; icon: string }[] = [
    { label: 'Est. Rows', value: formatRows(target.estimateRows), icon: 'fa-table-rows' },
    { label: 'Avg Row Size', value: `${target.avgRowSize} B`, icon: 'fa-ruler' },
  ];
  if (target.estimatedRowsRead != null) {
    metrics.push({ label: 'Est. Rows Read', value: formatRows(target.estimatedRowsRead), icon: 'fa-book-open' });
  }
  if (runtime) {
    metrics.push(
      { label: 'Actual Rows', value: formatRows(runtime.actualRows), icon: 'fa-table-list' },
      { label: 'Executions', value: runtime.actualExecutions.toString(), icon: 'fa-rotate' },
    );
    if (runtime.actualRowsRead != null) {
      metrics.push({ label: 'Actual Rows Read', value: formatRows(runtime.actualRowsRead), icon: 'fa-book-open' });
    }
    if (runtime.actualLogicalReads != null) {
      metrics.push({ label: 'Logical Reads', value: runtime.actualLogicalReads.toString(), icon: 'fa-database' });
    }
    if (runtime.actualPhysicalReads != null) {
      metrics.push({ label: 'Physical Reads', value: runtime.actualPhysicalReads.toString(), icon: 'fa-server' });
    }
    if (runtime.actualReadAheads != null && runtime.actualReadAheads > 0) {
      metrics.push({ label: 'Read-Aheads', value: runtime.actualReadAheads.toString(), icon: 'fa-forward' });
    }
  }
  return metrics;
});

const costPercentage = computed(() => {
  if (!selectedNode.value) return 0;
  return getNodeCostPercentage(selectedNode.value);
});

const costSeverity = computed(() => getCostSeverity(costPercentage.value));
const costColor = computed(() => getCostColor(costSeverity.value));

// Runtime metrics
const runtimeMetrics = computed(() => {
  if (!selectedNode.value) return [];
  const node = selectedNode.value;
  const runtime = node.runtimeInfo;
  
  const metrics = [
    { 
      label: 'Physical Op', 
      value: node.physicalOp,
      icon: 'fa-microchip'
    },
    { 
      label: 'Logical Op', 
      value: node.logicalOp,
      icon: 'fa-sitemap'
    },
    { 
      label: 'Est. Rows', 
      value: formatRows(node.estimateRows),
      icon: 'fa-table-rows'
    },
    { 
      label: 'Est. CPU', 
      value: node.estimateCPU.toFixed(6),
      icon: 'fa-gauge-high'
    },
    { 
      label: 'Est. I/O', 
      value: node.estimateIO.toFixed(6),
      icon: 'fa-hard-drive'
    },
    { 
      label: 'Subtree Cost', 
      value: node.estimatedTotalSubtreeCost.toFixed(6),
      icon: 'fa-calculator'
    },
  ];
  
  // Add runtime info if available
  if (runtime) {
    metrics.push(
      { 
        label: 'Actual Rows', 
        value: formatRows(runtime.actualRows),
        icon: 'fa-table-list'
      },
      { 
        label: 'Actual Time', 
        value: formatTime(runtime.actualElapsedMs),
        icon: 'fa-clock'
      },
      { 
        label: 'Actual CPU', 
        value: formatTime(runtime.actualCPUMs),
        icon: 'fa-bolt'
      },
      { 
        label: 'Executions', 
        value: runtime.actualExecutions.toString(),
        icon: 'fa-rotate'
      },
    );
    
    if (runtime.actualLogicalReads !== undefined) {
      metrics.push({ 
        label: 'Logical Reads', 
        value: runtime.actualLogicalReads.toString(),
        icon: 'fa-database'
      });
    }
    
    if (runtime.actualPhysicalReads !== undefined && runtime.actualPhysicalReads > 0) {
      metrics.push({ 
        label: 'Physical Reads', 
        value: runtime.actualPhysicalReads.toString(),
        icon: 'fa-server'
      });
    }
  }
  
  return metrics;
});

// Internal column SQL Server uses for partition elimination seeks (e.g. PtnId1000)
const PARTITION_ID_COLUMN = /^PtnId\d+$/i;

// Strip the parentheses ShowPlan puts around constants: "(2)" -> "2"
function stripConstParens(expr: string): string {
  return expr.replace(/^\((.*)\)$/, '$1');
}

// Partition IDs targeted by seek keys on the internal partition ID column
const partitionSeekValues = computed(() => {
  const seeks = selectedNode.value?.operationDetails.indexScan?.seekPredicates;
  if (!seeks) return [];
  const values: string[] = [];
  for (const seek of seeks) {
    for (const range of [seek.prefix, seek.startRange, seek.endRange]) {
      if (range && range.rangeColumns.some(c => PARTITION_ID_COLUMN.test(c.column))) {
        values.push(...range.rangeExpressions.map(stripConstParens));
      }
    }
  }
  return values;
});

const partitionInfo = computed(() => {
  const node = selectedNode.value;
  if (!node) return null;
  const accessed = node.runtimeInfo?.partitionsAccessed;
  const seekValues = partitionSeekValues.value;
  if (!node.partitioned && !accessed && seekValues.length === 0) return null;
  return {
    accessed,
    seekValues,
    rangesText: accessed
      ? accessed.ranges.map(r => (r.start === r.end ? `${r.start}` : `${r.start}-${r.end}`)).join(', ')
      : '',
  };
});

// Index details
const indexDetails = computed(() => {
  if (!selectedNode.value?.operationDetails.indexScan) return null;
  const scan = selectedNode.value.operationDetails.indexScan;
  return {
    table: scan.object.table,
    index: scan.object.index,
    indexKind: scan.object.indexKind,
    ordered: scan.ordered,
    direction: scan.scanDirection,
  };
});

// Output columns
const outputColumns = computed(() => {
  if (!selectedNode.value) return [];
  return selectedNode.value.outputColumns.map(col => {
    const parts = [];
    if (col.table) parts.push(col.table);
    parts.push(col.column);
    return parts.join('.');
  });
});

// Predicates and seek conditions
const predicates = computed(() => {
  if (!selectedNode.value) return [];
  const node = selectedNode.value;
  const results: { type: string; expression: string }[] = [];
  
  // Filter predicates
  if (node.operationDetails.filter?.predicate) {
    results.push({
      type: 'Filter',
      expression: node.operationDetails.filter.predicate
    });
  }
  
  // Seek predicates from index scan; seeks on the internal PtnId column are
  // partition elimination, not a user-visible index column
  const isPartitionRange = (range: { rangeColumns: { column: string }[] }) =>
    range.rangeColumns.length > 0 && range.rangeColumns.every(c => PARTITION_ID_COLUMN.test(c.column));
  const rangeColumnNames = (range: { rangeColumns: { column: string }[] }) =>
    range.rangeColumns.map(c => (PARTITION_ID_COLUMN.test(c.column) ? 'Partition ID' : c.column)).join(', ');

  if (node.operationDetails.indexScan?.seekPredicates) {
    for (const seek of node.operationDetails.indexScan.seekPredicates) {
      if (seek.prefix) {
        const cols = rangeColumnNames(seek.prefix);
        const exprs = seek.prefix.rangeExpressions.map(stripConstParens).join(', ');
        results.push({
          type: isPartitionRange(seek.prefix) ? 'Partition Elimination' : `Seek (${seek.prefix.scanType})`,
          expression: `${cols} = ${exprs}`,
        });
      }
      if (seek.startRange) {
        const cols = rangeColumnNames(seek.startRange);
        const exprs = seek.startRange.rangeExpressions.map(stripConstParens).join(', ');
        results.push({
          type: isPartitionRange(seek.startRange) ? 'Partition Elimination Start' : `Seek Start (${seek.startRange.scanType})`,
          expression: `${cols} ${seek.startRange.scanType} ${exprs}`,
        });
      }
      if (seek.endRange) {
        const cols = rangeColumnNames(seek.endRange);
        const exprs = seek.endRange.rangeExpressions.map(stripConstParens).join(', ');
        results.push({
          type: isPartitionRange(seek.endRange) ? 'Partition Elimination End' : `Seek End (${seek.endRange.scanType})`,
          expression: `${cols} ${seek.endRange.scanType} ${exprs}`,
        });
      }
    }
  }

  // Residual predicate on index seek/scan
  if (node.operationDetails.indexScan?.predicate) {
    results.push({ type: 'Residual', expression: node.operationDetails.indexScan.predicate });
  }
  
  // Nested loops outer references
  if (node.operationDetails.nestedLoops?.outerReferences) {
    const refs = node.operationDetails.nestedLoops.outerReferences;
    if (refs.length > 0) {
      results.push({
        type: 'Outer References',
        expression: refs.map(r => r.column).join(', ')
      });
    }
  }
  
  return results;
});

// Helper to flatten object for dynamic display
function formatProperties(node: RelOp): { key: string; value: string }[] {
  const processObject = (obj: unknown, prefix = ''): { key: string; value: string }[] => {
    let result: { key: string; value: string }[] = [];
    
    if (!obj || typeof obj !== 'object') return [];

    Object.keys(obj).sort().forEach(key => {
      // Skip internal navigation properties and already displayed specialized structures if redundant
      if (key === 'children' || key === 'parent') return;
      
      const value = (obj as Record<string, unknown>)[key];
      const currentKey = prefix ? `${prefix}.${key}` : key;
      
      if (value === null || value === undefined) {
        return;
      } 
      
      if (Array.isArray(value)) {
        if (value.length === 0) return;
        
        // Check if array of primitives
        if (value.length > 0 && typeof value[0] !== 'object') {
           result.push({ key: currentKey, value: value.join(', ') });
        } else {
           // Recursively process array items
           value.forEach((item, index) => {
               result = result.concat(processObject(item, `${currentKey}[${index}]`));
           });
        }
      } else if (typeof value === 'object') {
        result = result.concat(processObject(value, currentKey));
      } else {
        result.push({ key: currentKey, value: String(value) });
      }
    });
    
    return result;
  };

  return processObject(node);
}

const formattedProperties = computed(() => selectedNode.value ? formatProperties(selectedNode.value) : []);

const filteredProperties = computed(() => {
  const term = searchTerm.value.trim().toLowerCase();
  if (!term) return formattedProperties.value;
  return formattedProperties.value.filter(
    prop => prop.key.toLowerCase().includes(term) || String(prop.value).toLowerCase().includes(term)
  );
});

const copied = ref('');
const copyText = async (text: string, label: string) => {
  await navigator.clipboard.writeText(text);
  copied.value = label;
  setTimeout(() => { copied.value = ''; }, 1500);
};

// Chain from the plan root down to the selected node, so the copied text shows
// which operator is driving this one (e.g. a nested loop probing it per outer row).
const ancestorChain = computed<RelOp[]>(() => {
  if (!selectedNode.value) return [];
  const parentOf = new Map<number, RelOp>();
  for (const node of allNodes.value) {
    for (const child of node.children) parentOf.set(child.nodeId, node);
  }
  const chain: RelOp[] = [];
  let current: RelOp | undefined = parentOf.get(selectedNode.value.nodeId);
  const seen = new Set<number>();
  while (current && !seen.has(current.nodeId)) {
    seen.add(current.nodeId);
    chain.unshift(current);
    current = parentOf.get(current.nodeId);
  }
  return chain;
});

function describeOp(node: RelOp): string {
  const parts = [`${node.physicalOp} (Node ${node.nodeId})`];
  parts.push(`est. rows ${formatRows(node.estimateRows)}`);
  if (node.runtimeInfo) {
    parts.push(`actual rows ${formatRows(node.runtimeInfo.actualRows)}`);
    parts.push(`${node.runtimeInfo.actualExecutions} execution(s)`);
  }
  return parts.join(', ');
}

function buildNodeMarkdown(): string {
  const node = selectedNode.value;
  if (!node) return '';

  const statement = state.selectedStatement;
  const plan = statement?.queryPlan;

  const lines: string[] = [];
  lines.push(`# ${node.physicalOp} (Node ${node.nodeId})`);

  // Query first: the node alone doesn't explain why it's being executed.
  if (statement) {
    lines.push('');
    lines.push('## Query');
    lines.push(`- **Statement Type:** ${statement.statementType}`);
    lines.push(`- **Statement Subtree Cost:** ${statement.statementSubTreeCost.toFixed(6)}`);
    if (statement.statementOptmLevel) lines.push(`- **Optimization Level:** ${statement.statementOptmLevel}`);
    if (plan) lines.push(`- **Degree of Parallelism:** ${plan.degreeOfParallelism}`);
    lines.push('');
    lines.push('```sql');
    lines.push(statement.statementText.trim());
    lines.push('```');
  }

  if (plan?.parameters?.length) {
    lines.push('');
    lines.push('## Parameters');
    for (const p of plan.parameters) {
      const values = [];
      if (p.compiledValue) values.push(`compiled: ${p.compiledValue}`);
      if (p.runtimeValue) values.push(`runtime: ${p.runtimeValue}`);
      lines.push(`- **${p.column}** (${p.dataType})${values.length ? ` - ${values.join(', ')}` : ''}`);
    }
  }

  // Plan context: what feeds this node and what consumes its rows.
  lines.push('');
  lines.push('## Plan Context');
  if (ancestorChain.value.length > 0) {
    lines.push('Ancestors (root first):');
    ancestorChain.value.forEach((ancestor, i) => {
      lines.push(`${'  '.repeat(i)}- ${describeOp(ancestor)}`);
    });
    lines.push(`${'  '.repeat(ancestorChain.value.length)}- **${describeOp(node)}** <- selected`);
  } else {
    lines.push(`- **${describeOp(node)}** <- selected (plan root)`);
  }
  if (node.children.length > 0) {
    lines.push('');
    lines.push('Children:');
    for (const child of node.children) lines.push(`- ${describeOp(child)}`);
  }

  lines.push('');
  lines.push('## Node');
  lines.push(`- **Cost Percentage:** ${costPercentage.value.toFixed(1)}%`);

  lines.push('');
  lines.push('## Metrics');
  for (const m of runtimeMetrics.value) {
    lines.push(`- **${m.label}:** ${m.value}`);
  }

  if (partitionInfo.value) {
    const pi = partitionInfo.value;
    lines.push('');
    lines.push('## Partitioning');
    lines.push('- Operator accesses a partitioned object.');
    if (pi.seekValues.length) {
      lines.push(`- **Partition elimination (seek):** Partition ID = ${pi.seekValues.join(', ')}`);
    }
    if (pi.accessed) {
      lines.push(`- **Partitions accessed (actual):** ${pi.accessed.partitionCount}`);
      if (pi.rangesText) lines.push(`- **Partition ranges:** ${pi.rangesText}`);
    }
  }

  if (indexDetails.value) {
    lines.push('');
    lines.push('## Index Information');
    lines.push(`- **Table:** ${indexDetails.value.table}`);
    if (indexDetails.value.index) lines.push(`- **Index:** ${indexDetails.value.index}`);
    if (indexDetails.value.indexKind) lines.push(`- **Type:** ${indexDetails.value.indexKind}`);
    lines.push(`- **Ordered:** ${indexDetails.value.ordered ? 'Yes' : 'No'}`);
    if (indexDetails.value.direction) lines.push(`- **Scan Direction:** ${indexDetails.value.direction}`);
  }

  if (outputColumns.value.length > 0) {
    lines.push('');
    lines.push('## Output Columns');
    for (const col of outputColumns.value) lines.push(`- ${col}`);
  }

  if (predicates.value.length > 0) {
    lines.push('');
    lines.push('## Predicates');
    for (const p of predicates.value) lines.push(`- **${p.type}:** ${p.expression}`);
  }

  if (node.parallel) {
    lines.push('');
    lines.push('## Parallel Execution');
    lines.push('- This operator runs in parallel.');
  }

  if (plan?.missingIndexes?.length) {
    lines.push('');
    lines.push('## Missing Indexes (optimizer suggestions for this statement)');
    for (const mi of plan.missingIndexes) {
      lines.push(`- **${mi.table}** (impact ${mi.impact.toFixed(1)}%)`);
      if (mi.equalityColumns.length) lines.push(`  - Equality: ${mi.equalityColumns.join(', ')}`);
      if (mi.inequalityColumns.length) lines.push(`  - Inequality: ${mi.inequalityColumns.join(', ')}`);
      if (mi.includeColumns.length) lines.push(`  - Include: ${mi.includeColumns.join(', ')}`);
    }
  }

  const warnings = plan?.warnings;
  if (warnings) {
    const warningLines: string[] = [];
    if (warnings.noJoinPredicate) warningLines.push('- No join predicate');
    if (warnings.unmatchedIndexes) warningLines.push('- Unmatched indexes');
    for (const spill of warnings.spills ?? []) warningLines.push(`- Spill: ${spill.kind}`);
    for (const c of warnings.planAffectingConverts ?? []) {
      warningLines.push(`- Plan-affecting convert (${c.convertIssue}): ${c.expression}`);
    }
    for (const col of warnings.columnsWithNoStatistics ?? []) {
      warningLines.push(`- No statistics: ${[col.table, col.column].filter(Boolean).join('.')}`);
    }
    if (warningLines.length) {
      lines.push('');
      lines.push('## Statement Warnings');
      lines.push(...warningLines);
    }
  }

  lines.push('');
  lines.push('## All Properties');
  for (const prop of formattedProperties.value) {
    lines.push(`- **${prop.key}:** ${prop.value}`);
  }

  return lines.join('\n');
}

function buildSubtreeMarkdown(): string {
  const root = selectedNode.value;
  if (!root) return '';

  const lines = [
    'Analyze this SQL Server execution plan subtree, including the selected node and all its descendants. Use the query and ancestor context to explain how the operators interact, identify likely bottlenecks, and suggest improvements supported by the supplied metrics. Distinguish estimates from actual runtime data and state when more information is needed.',
    '',
    buildNodeMarkdown(),
    '',
    '## Descendant Nodes',
    'Each node below includes all its properties. Parent and child IDs describe the tree within the selected statement.',
  ];

  const pending = root.children.map(node => ({ node, parentId: root.nodeId })).reverse();
  while (pending.length > 0) {
    const { node, parentId } = pending.pop()!;
    lines.push('', `### ${node.physicalOp} (Node ${node.nodeId})`);
    lines.push(`- **Parent Node:** ${parentId}`);
    lines.push(`- **Child Nodes:** ${node.children.length ? node.children.map(child => child.nodeId).join(', ') : 'None'}`);
    lines.push(`- **Cost Percentage:** ${getNodeCostPercentage(node).toFixed(1)}%`);
    for (const prop of formatProperties(node)) {
      lines.push(`- **${prop.key}:** ${prop.value}`);
    }
    for (let i = node.children.length - 1; i >= 0; i--) {
      pending.push({ node: node.children[i], parentId: node.nodeId });
    }
  }

  if (root.children.length === 0) lines.push('The selected node has no child nodes.');
  return lines.join('\n');
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function highlightText(text: string, term: string): string {
  const escaped = escapeHtml(text);
  if (!term.trim()) return escaped;
  const escapedTerm = escapeHtml(term).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return escaped.replace(
    new RegExp(escapedTerm, 'gi'),
    match => `<mark class="bg-yellow-400/40 text-yellow-100 rounded px-px">${match}</mark>`
  );
}
</script>

<template>
  <div class="h-full flex flex-col bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div class="px-4 py-3 bg-slate-700 border-b border-slate-600">
      <h3 class="flex flex-wrap items-center gap-2 text-lg font-bold text-white">
        <i class="fa-solid fa-info-circle text-cyan-400"></i>
        Node Details
        <button
          v-if="selectedNode"
          @click="copyText(buildNodeMarkdown(), 'node')"
          class="ml-auto shrink-0 flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors"
          :class="copied === 'node' ? 'bg-green-600/30 text-green-300' : 'bg-indigo-600/30 text-indigo-300 hover:bg-indigo-600/50 hover:text-indigo-200'"
          title="Copy node details, query and plan context formatted for LLM analysis"
        >
          <i :class="copied === 'node' ? 'fa-solid fa-check' : 'fa-solid fa-robot'"></i>
          {{ copied === 'node' ? 'Copied!' : 'Copy AI' }}
        </button>
        <button
          v-if="selectedNode"
          @click="copyText(buildSubtreeMarkdown(), 'subtree')"
          class="shrink-0 flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cyan-400"
          :class="copied === 'subtree' ? 'bg-green-600/30 text-green-300' : 'bg-indigo-600/30 text-indigo-300 hover:bg-indigo-600/50 hover:text-indigo-200'"
          title="Copy the selected node and all descendants, query and plan context with AI analysis instructions"
        >
          <i :class="copied === 'subtree' ? 'fa-solid fa-check' : 'fa-solid fa-sitemap'" aria-hidden="true"></i>
          {{ copied === 'subtree' ? 'Copied!' : 'Copy AI + children' }}
        </button>
      </h3>
    </div>
    
    <!-- Empty State -->
    <div v-if="!selectedNode && !selectedEdge" class="flex-1 flex flex-col items-center justify-center text-slate-500">
      <i class="fa-solid fa-hand-pointer text-5xl mb-4 animate-pulse"></i>
      <p class="text-sm">Click a node or edge to view details</p>
    </div>

    <!-- Edge Details -->
    <div v-else-if="selectedEdge" class="flex-1 overflow-y-auto p-4 space-y-4">
      <!-- Edge Header -->
      <div class="bg-slate-700 rounded-xl p-4">
        <div class="flex items-center gap-2 mb-3">
          <i class="fa-solid fa-arrow-right text-cyan-400"></i>
          <h4 class="font-bold text-white text-sm">Data Flow</h4>
        </div>
        <div class="text-sm text-slate-300">
          <span class="font-mono">{{ selectedEdge.target.physicalOp }}</span>
          <i class="fa-solid fa-arrow-right text-slate-500 mx-2"></i>
          <span class="font-mono">{{ selectedEdge.source.physicalOp }}</span>
        </div>
      </div>

      <!-- Edge Metrics Grid -->
      <div class="grid grid-cols-2 gap-2">
        <div
          v-for="metric in edgeMetrics"
          :key="metric.label"
          class="bg-slate-700/50 rounded-lg p-3"
        >
          <div class="flex items-center gap-2 text-xs text-slate-400 mb-1">
            <i :class="'fa-solid ' + metric.icon" class="w-3"></i>
            {{ metric.label }}
          </div>
          <div class="text-sm font-semibold text-slate-200 truncate" :title="metric.value">
            {{ metric.value }}
          </div>
        </div>
      </div>
    </div>

    <!-- Node Details -->
    <div v-else-if="selectedNode" class="flex-1 overflow-y-auto p-4 space-y-4">
      <!-- Node Header -->
      <div class="bg-slate-700 rounded-xl p-4">
        <div class="flex items-center gap-3 mb-3">
          <div 
            class="w-10 h-10 rounded-lg flex items-center justify-center"
            :style="{ backgroundColor: costColor + '20', borderColor: costColor }"
            style="border-width: 2px;"
          >
            <i 
              :class="'fa-solid ' + getOperatorIcon(selectedNode.physicalOp)"
              :style="{ color: costColor }"
            ></i>
          </div>
          <div>
            <h4 class="font-bold text-white">{{ selectedNode.physicalOp }}</h4>
            <p class="text-xs text-slate-400">Node ID: {{ selectedNode.nodeId }}</p>
          </div>
        </div>
        
        <!-- Cost Bar -->
        <div class="mt-3">
          <div class="flex justify-between text-xs mb-1">
            <span class="text-slate-400">Cost Percentage</span>
            <span :style="{ color: costColor }" class="font-bold">{{ costPercentage.toFixed(1) }}%</span>
          </div>
          <div class="h-2 bg-slate-600 rounded-full overflow-hidden">
            <div 
              class="h-full rounded-full transition-all duration-500"
              :style="{ width: costPercentage + '%', backgroundColor: costColor }"
            ></div>
          </div>
        </div>
      </div>

      <!-- Partitioning (shown prominently: partition access drives IO on partitioned tables) -->
      <div v-if="partitionInfo" class="bg-violet-500/10 border border-violet-500/30 rounded-xl p-4">
        <div class="flex items-center gap-2 text-violet-300 mb-2">
          <i class="fa-solid fa-table-cells-large"></i>
          <span class="text-sm font-semibold">Partitioned Table Access</span>
        </div>
        <div class="space-y-2 text-sm">
          <div v-if="partitionInfo.accessed" class="flex justify-between">
            <span class="text-slate-400">Partitions Accessed (actual)</span>
            <span class="text-slate-200 font-semibold">{{ partitionInfo.accessed.partitionCount }}</span>
          </div>
          <div v-if="partitionInfo.rangesText" class="flex justify-between">
            <span class="text-slate-400">Partition Range(s)</span>
            <span class="text-slate-200 font-mono">{{ partitionInfo.rangesText }}</span>
          </div>
          <div v-if="partitionInfo.seekValues.length" class="flex justify-between">
            <span class="text-slate-400">Partition Elimination (seek)</span>
            <span class="text-slate-200 font-mono">Partition ID = {{ partitionInfo.seekValues.join(', ') }}</span>
          </div>
          <div v-if="!partitionInfo.accessed && !partitionInfo.seekValues.length" class="text-slate-300">
            Operator accesses a partitioned object.
          </div>
        </div>
      </div>

      <!-- Metrics Grid -->
      <CollapsiblePanel title="Metrics" icon="fa-chart-bar" icon-color="text-cyan-400" :badge="runtimeMetrics.length">
        <div class="grid grid-cols-2 gap-2">
          <div
            v-for="metric in runtimeMetrics"
            :key="metric.label"
            class="bg-slate-700/50 rounded-lg p-3"
          >
            <div class="flex items-center gap-2 text-xs text-slate-400 mb-1">
              <i :class="'fa-solid ' + metric.icon" class="w-3"></i>
              {{ metric.label }}
            </div>
            <div class="text-sm font-semibold text-slate-200 truncate" :title="metric.value">
              {{ metric.value }}
            </div>
          </div>
        </div>
      </CollapsiblePanel>
      
      <!-- Index Details -->
      <CollapsiblePanel v-if="indexDetails" title="Index Information" icon="fa-key" icon-color="text-amber-400" panel-class="bg-slate-700/50 rounded-xl p-4">
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-slate-400">Table</span>
            <span class="text-slate-200 font-mono">{{ indexDetails.table }}</span>
          </div>
          <div v-if="indexDetails.index" class="flex flex-col gap-0.5">
            <span class="text-slate-400">Index</span>
            <span class="text-slate-200 font-mono break-all">{{ indexDetails.index }}</span>
          </div>
          <div v-if="indexDetails.indexKind" class="flex justify-between">
            <span class="text-slate-400">Type</span>
            <span class="text-slate-200">{{ indexDetails.indexKind }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-slate-400">Ordered</span>
            <span :class="indexDetails.ordered ? 'text-green-400' : 'text-slate-400'">
              {{ indexDetails.ordered ? 'Yes' : 'No' }}
            </span>
          </div>
        </div>
      </CollapsiblePanel>
      
      <!-- Output Columns -->
      <CollapsiblePanel v-if="outputColumns.length > 0" title="Output Columns" icon="fa-columns" icon-color="text-blue-400" :badge="outputColumns.length" panel-class="bg-slate-700/50 rounded-xl p-4">
        <div class="flex flex-wrap gap-1">
          <span
            v-for="col in outputColumns.slice(0, 10)"
            :key="col"
            class="px-2 py-1 bg-slate-600 rounded text-xs text-slate-300 font-mono"
          >
            {{ col }}
          </span>
          <span
            v-if="outputColumns.length > 10"
            class="px-2 py-1 text-xs text-slate-500"
          >
            +{{ outputColumns.length - 10 }} more
          </span>
        </div>
      </CollapsiblePanel>
      
      <!-- Predicates -->
      <CollapsiblePanel v-if="predicates.length > 0" title="Predicates" icon="fa-filter" icon-color="text-green-400" :badge="predicates.length" panel-class="bg-slate-700/50 rounded-xl p-4">
        <div class="space-y-2">
          <div
            v-for="(pred, idx) in predicates"
            :key="idx"
            class="bg-slate-600/50 rounded-lg p-2"
          >
            <div class="text-xs text-slate-400 mb-1">{{ pred.type }}</div>
            <div class="text-xs font-mono text-slate-200 break-all">{{ pred.expression }}</div>
          </div>
        </div>
      </CollapsiblePanel>
      
      <!-- Parallel Execution -->
      <div v-if="selectedNode.parallel" class="bg-amber-500/10 border border-amber-500/30 rounded-xl p-4">
        <div class="flex items-center gap-2 text-amber-400">
          <i class="fa-solid fa-network-wired"></i>
          <span class="text-sm font-semibold">Parallel Execution</span>
        </div>
      </div>

      <!-- All Properties (Dynamic) -->
      <CollapsiblePanel
        title="All Properties"
        icon="fa-list-ul"
        icon-color="text-indigo-400"
        :badge="searchTerm ? `${filteredProperties.length} / ${formattedProperties.length}` : formattedProperties.length"
        :collapsed="true"
        panel-class="bg-slate-700/50 rounded-xl p-4 mt-4"
      >
        <!-- Search Input -->
        <div class="relative mb-3">
          <i class="fa-solid fa-magnifying-glass absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400 text-xs pointer-events-none"></i>
          <input
            v-model="searchTerm"
            type="text"
            placeholder="Search properties..."
            class="w-full bg-slate-600/60 border border-slate-500/50 rounded-lg pl-7 pr-7 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-400/70 focus:ring-1 focus:ring-indigo-400/30"
          />
          <button
            v-if="searchTerm"
            @click="searchTerm = ''"
            class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200 text-xs"
            aria-label="Clear search"
          >
            <i class="fa-solid fa-xmark"></i>
          </button>
        </div>
        <div v-if="filteredProperties.length > 0" class="space-y-1 overflow-x-auto">
          <div
            v-for="prop in filteredProperties"
            :key="prop.key"
            class="grid grid-cols-[1fr_2fr] gap-2 text-xs border-b border-slate-600/50 py-1 hover:bg-slate-600/30"
          >
            <span class="text-slate-400 font-mono break-all" v-html="highlightText(prop.key, searchTerm)"></span>
            <span class="text-slate-200 font-mono break-all" v-html="highlightText(String(prop.value), searchTerm)"></span>
          </div>
        </div>
        <div v-else class="text-xs text-slate-500 text-center py-3">
          No properties match "{{ searchTerm }}"
        </div>
      </CollapsiblePanel>
    </div>
  </div>
</template>
