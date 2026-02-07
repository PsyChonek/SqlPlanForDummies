<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState } from '../composables/planState';
import { 
  getOperatorIcon, 
  getCostSeverity, 
  getCostColor, 
  formatTime, 
  formatRows
} from '../types/sqlplan';

const { state, getNodeCostPercentage } = usePlanState();

const selectedNode = computed(() => state.selectedNode);
const selectedEdge = computed(() => state.selectedEdge);

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
  
  // Seek predicates from index scan
  if (node.operationDetails.indexScan?.seekPredicates) {
    for (const seek of node.operationDetails.indexScan.seekPredicates) {
      if (seek.prefix) {
        const columns = seek.prefix.rangeColumns.map(c => c.column).join(', ');
        const expressions = seek.prefix.rangeExpressions.join(', ');
        results.push({
          type: `Seek (${seek.prefix.scanType})`,
          expression: `${columns} = ${expressions}`
        });
      }
    }
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
</script>

<template>
  <div class="h-full flex flex-col bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div class="px-4 py-3 bg-slate-700 border-b border-slate-600">
      <h3 class="flex items-center gap-2 text-lg font-bold text-white">
        <i class="fa-solid fa-info-circle text-cyan-400"></i>
        Node Details
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
      
      <!-- Metrics Grid -->
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
      
      <!-- Index Details -->
      <div v-if="indexDetails" class="bg-slate-700/50 rounded-xl p-4">
        <h5 class="flex items-center gap-2 text-sm font-semibold text-slate-300 mb-3">
          <i class="fa-solid fa-key text-amber-400"></i>
          Index Information
        </h5>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-slate-400">Table</span>
            <span class="text-slate-200 font-mono">{{ indexDetails.table }}</span>
          </div>
          <div v-if="indexDetails.index" class="flex justify-between">
            <span class="text-slate-400">Index</span>
            <span class="text-slate-200 font-mono">{{ indexDetails.index }}</span>
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
      </div>
      
      <!-- Output Columns -->
      <div v-if="outputColumns.length > 0" class="bg-slate-700/50 rounded-xl p-4">
        <h5 class="flex items-center gap-2 text-sm font-semibold text-slate-300 mb-3">
          <i class="fa-solid fa-columns text-blue-400"></i>
          Output Columns ({{ outputColumns.length }})
        </h5>
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
      </div>
      
      <!-- Predicates -->
      <div v-if="predicates.length > 0" class="bg-slate-700/50 rounded-xl p-4">
        <h5 class="flex items-center gap-2 text-sm font-semibold text-slate-300 mb-3">
          <i class="fa-solid fa-filter text-green-400"></i>
          Predicates
        </h5>
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
      </div>
      
      <!-- Parallel Execution -->
      <div v-if="selectedNode.parallel" class="bg-amber-500/10 border border-amber-500/30 rounded-xl p-4">
        <div class="flex items-center gap-2 text-amber-400">
          <i class="fa-solid fa-network-wired"></i>
          <span class="text-sm font-semibold">Parallel Execution</span>
        </div>
      </div>
    </div>
  </div>
</template>
