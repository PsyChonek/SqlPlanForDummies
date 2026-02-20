<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState } from '../composables/planState';
import { flattenRelOps } from '../composables/sqlPlanParser';
import type { RelOp } from '../types/sqlplan';

const props = withDefaults(defineProps<{ showHeader?: boolean }>(), { showHeader: true });

const { state, getNodeCostPercentage } = usePlanState();

interface Issue {
  severity: 'critical' | 'warning' | 'info';
  title: string;
  description: string;
  nodeId?: number;
  impact: number; // 0-100
}

// Analyze the execution plan for issues
const issues = computed((): Issue[] => {
  if (!state.selectedStatement) return [];
  
  const results: Issue[] = [];
  const nodes = flattenRelOps(state.selectedStatement.queryPlan.relOp);
  
  for (const node of nodes) {
    const costPct = getNodeCostPercentage(node);
    
    // Table Scan warnings
    if (node.physicalOp === 'Table Scan' && node.estimateRows > 1000) {
      results.push({
        severity: node.estimateRows > 10000 ? 'critical' : 'warning',
        title: 'Table Scan Detected',
        description: `Table scan on ${getTableName(node)} processing ${node.estimateRows.toLocaleString()} estimated rows. Consider adding an index.`,
        nodeId: node.nodeId,
        impact: Math.min(100, costPct * 2),
      });
    }
    
    // Clustered Index Scan on large tables
    if (node.physicalOp === 'Clustered Index Scan' && node.estimateRows > 10000) {
      results.push({
        severity: 'warning',
        title: 'Clustered Index Scan',
        description: `Full clustered index scan on ${getTableName(node)} with ${node.estimateRows.toLocaleString()} rows. A non-clustered index might improve performance.`,
        nodeId: node.nodeId,
        impact: Math.min(100, costPct * 1.5),
      });
    }
    
    // Key Lookup concerns
    if (node.physicalOp === 'Key Lookup' || node.physicalOp === 'RID Lookup') {
      const executions = node.runtimeInfo?.actualExecutions || node.estimateRows;
      if (executions > 100) {
        results.push({
          severity: executions > 1000 ? 'critical' : 'warning',
          title: 'Key Lookup Operations',
          description: `${node.physicalOp} executed ${executions.toLocaleString()} times. Consider adding columns to the index to avoid lookups.`,
          nodeId: node.nodeId,
          impact: Math.min(100, costPct * 2),
        });
      }
    }
    
    // Sort operations with high cost
    if (node.physicalOp === 'Sort' && costPct > 15) {
      results.push({
        severity: costPct > 30 ? 'critical' : 'warning',
        title: 'Expensive Sort Operation',
        description: `Sort operation consuming ${costPct.toFixed(1)}% of query cost. Consider an index that provides pre-sorted data.`,
        nodeId: node.nodeId,
        impact: costPct,
      });
    }
    
    // Hash Match with high memory
    if (node.physicalOp === 'Hash Match' && node.estimateRows > 100000) {
      results.push({
        severity: 'warning',
        title: 'Large Hash Operation',
        description: `Hash Match processing ${node.estimateRows.toLocaleString()} rows may require significant memory. Consider if a Merge Join would be more efficient.`,
        nodeId: node.nodeId,
        impact: Math.min(100, costPct * 1.5),
      });
    }
    
    // High cost single node
    if (costPct > 50) {
      results.push({
        severity: 'critical',
        title: 'High Cost Operation',
        description: `${node.physicalOp} accounts for ${costPct.toFixed(1)}% of total query cost. This is the primary optimization target.`,
        nodeId: node.nodeId,
        impact: costPct,
      });
    }
    
    // Row estimate vs actual mismatch
    if (node.runtimeInfo && node.estimateRows > 0) {
      const ratio = node.runtimeInfo.actualRows / node.estimateRows;
      if (ratio > 10 || ratio < 0.1) {
        results.push({
          severity: 'warning',
          title: 'Estimate Mismatch',
          description: `${node.physicalOp}: Estimated ${node.estimateRows.toFixed(0)} rows but got ${node.runtimeInfo.actualRows}. Statistics may be outdated.`,
          nodeId: node.nodeId,
          impact: Math.min(100, Math.abs(Math.log10(ratio)) * 20),
        });
      }
    }
  }
  
  // Sort by impact
  return results.sort((a, b) => b.impact - a.impact).slice(0, 10);
});

// Get table name from node
function getTableName(node: RelOp): string {
  const indexScan = node.operationDetails.indexScan;
  if (indexScan?.object.table) {
    return indexScan.object.table;
  }
  return 'unknown table';
}

// Plan summary stats
const planStats = computed(() => {
  if (!state.selectedStatement) return null;
  
  const nodes = flattenRelOps(state.selectedStatement.queryPlan.relOp);
  const totalNodes = nodes.length;
  const parallelNodes = nodes.filter(n => n.parallel).length;
  const scanNodes = nodes.filter(n => n.physicalOp.includes('Scan')).length;
  const seekNodes = nodes.filter(n => n.physicalOp.includes('Seek')).length;
  
  // Calculate total actual time if available
  let totalActualTime = 0;
  let hasRuntimeInfo = false;
  for (const node of nodes) {
    if (node.runtimeInfo) {
      hasRuntimeInfo = true;
      totalActualTime = Math.max(totalActualTime, node.runtimeInfo.actualElapsedMs);
    }
  }
  
  return {
    totalNodes,
    parallelNodes,
    scanNodes,
    seekNodes,
    totalCost: state.selectedStatement.statementSubTreeCost,
    actualTime: hasRuntimeInfo ? totalActualTime : null,
  };
});

const getSeverityIcon = (severity: string) => {
  switch (severity) {
    case 'critical': return 'fa-circle-xmark';
    case 'warning': return 'fa-triangle-exclamation';
    default: return 'fa-circle-info';
  }
};

const getSeverityColor = (severity: string) => {
  switch (severity) {
    case 'critical': return 'text-red-400';
    case 'warning': return 'text-amber-400';
    default: return 'text-blue-400';
  }
};

const getSeverityBg = (severity: string) => {
  switch (severity) {
    case 'critical': return 'bg-red-500/10 border-red-500/30';
    case 'warning': return 'bg-amber-500/10 border-amber-500/30';
    default: return 'bg-blue-500/10 border-blue-500/30';
  }
};

defineExpose({ issueCount: computed(() => issues.value.length) });
</script>

<template>
  <div class="h-full flex flex-col bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div v-if="props.showHeader" class="px-4 py-3 bg-slate-700 border-b border-slate-600 flex items-center justify-between">
      <h3 class="flex items-center gap-2 text-lg font-bold text-white">
        <i class="fa-solid fa-microscope text-purple-400"></i>
        Plan Analysis
      </h3>
      <span v-if="issues.length > 0" class="px-2 py-0.5 bg-amber-500/20 text-amber-400 text-xs font-semibold rounded-full">
        {{ issues.length }} issue{{ issues.length !== 1 ? 's' : '' }}
      </span>
    </div>
    
    <!-- Empty State -->
    <div v-if="!state.selectedStatement" class="flex-1 flex flex-col items-center justify-center text-slate-500">
      <i class="fa-solid fa-chart-line text-5xl mb-4"></i>
      <p class="text-sm">Load a plan to see analysis</p>
    </div>
    
    <!-- Content -->
    <div v-else class="flex-1 overflow-y-auto p-4 space-y-4">
      <!-- Plan Stats -->
      <div v-if="planStats" class="grid grid-cols-3 gap-2">
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold text-white">{{ planStats.totalNodes }}</div>
          <div class="text-xs text-slate-400">Operators</div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold text-green-400">{{ planStats.seekNodes }}</div>
          <div class="text-xs text-slate-400">Seeks</div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold" :class="planStats.scanNodes > 3 ? 'text-amber-400' : 'text-slate-300'">
            {{ planStats.scanNodes }}
          </div>
          <div class="text-xs text-slate-400">Scans</div>
        </div>
      </div>
      
      <!-- Timing Info -->
      <div v-if="planStats?.actualTime !== null && planStats?.actualTime !== undefined" class="bg-slate-700/50 rounded-lg p-3">
        <div class="flex items-center justify-between">
          <span class="text-sm text-slate-400">
            <i class="fa-solid fa-stopwatch mr-1"></i>
            Actual Execution Time
          </span>
          <span class="font-mono font-bold text-white">
            {{ planStats?.actualTime }}ms
          </span>
        </div>
      </div>
      
      <!-- Issues List -->
      <div v-if="issues.length > 0" class="space-y-2">
        <h4 class="text-sm font-semibold text-slate-400 flex items-center gap-2">
          <i class="fa-solid fa-list-check"></i>
          Detected Issues
        </h4>
        
        <div 
          v-for="(issue, idx) in issues"
          :key="idx"
          class="border rounded-lg p-3"
          :class="getSeverityBg(issue.severity)"
        >
          <div class="flex items-start gap-2">
            <i 
              :class="['fa-solid', getSeverityIcon(issue.severity), getSeverityColor(issue.severity)]"
              class="mt-0.5"
            ></i>
            <div class="flex-1 min-w-0">
              <div class="font-semibold text-sm text-slate-200">{{ issue.title }}</div>
              <p class="text-xs text-slate-400 mt-1">{{ issue.description }}</p>
              
              <!-- Impact bar -->
              <div class="mt-2">
                <div class="flex justify-between text-xs mb-1">
                  <span class="text-slate-500">Impact</span>
                  <span :class="getSeverityColor(issue.severity)">{{ issue.impact.toFixed(0) }}%</span>
                </div>
                <div class="h-1 bg-slate-600 rounded-full overflow-hidden">
                  <div 
                    class="h-full rounded-full"
                    :style="{ width: issue.impact + '%' }"
                    :class="{
                      'bg-red-500': issue.severity === 'critical',
                      'bg-amber-500': issue.severity === 'warning',
                      'bg-blue-500': issue.severity === 'info'
                    }"
                  ></div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      
      <!-- No Issues -->
      <div v-else class="bg-green-500/10 border border-green-500/30 rounded-lg p-4 text-center">
        <i class="fa-solid fa-circle-check text-3xl text-green-400 mb-2"></i>
        <p class="text-sm text-green-300 font-medium">No significant issues detected</p>
        <p class="text-xs text-slate-400 mt-1">The execution plan appears to be well-optimized</p>
      </div>
      
      <!-- Parallel Execution Info -->
      <div v-if="planStats && planStats.parallelNodes > 0" class="bg-blue-500/10 border border-blue-500/30 rounded-lg p-3">
        <div class="flex items-center gap-2 text-blue-400">
          <i class="fa-solid fa-diagram-project"></i>
          <span class="text-sm font-medium">
            {{ planStats?.parallelNodes }} parallel operator{{ planStats?.parallelNodes !== 1 ? 's' : '' }}
          </span>
        </div>
        <p class="text-xs text-slate-400 mt-1">
          DOP: {{ state.selectedStatement?.queryPlan.degreeOfParallelism || 1 }}
        </p>
      </div>
    </div>
  </div>
</template>
