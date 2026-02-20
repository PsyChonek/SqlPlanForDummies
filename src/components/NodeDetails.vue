<script setup lang="ts">
import { computed, ref, watch } from 'vue';
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

// Helper to flatten object for dynamic display
const formattedProperties = computed(() => {
  if (!selectedNode.value) return [];
  
  const processObject = (obj: any, prefix = ''): { key: string; value: any }[] => {
    let result: { key: string; value: any }[] = [];
    
    if (!obj || typeof obj !== 'object') return [];

    Object.keys(obj).sort().forEach(key => {
      // Skip internal navigation properties and already displayed specialized structures if redundant
      if (key === 'children' || key === 'parent') return;
      
      const value = obj[key];
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

  return processObject(selectedNode.value);
});

const filteredProperties = computed(() => {
  const term = searchTerm.value.trim().toLowerCase();
  if (!term) return formattedProperties.value;
  return formattedProperties.value.filter(
    prop => prop.key.toLowerCase().includes(term) || String(prop.value).toLowerCase().includes(term)
  );
});

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

      <!-- All Properties (Dynamic) -->
      <div class="bg-slate-700/50 rounded-xl p-4 mt-4">
        <div class="flex items-center gap-2 mb-3">
          <i class="fa-solid fa-list-ul text-indigo-400"></i>
          <h5 class="text-sm font-semibold text-slate-300 flex-1">All Properties</h5>
          <span v-if="searchTerm" class="text-xs text-slate-500">{{ filteredProperties.length }} / {{ formattedProperties.length }}</span>
        </div>
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
      </div>
    </div>
  </div>
</template>
