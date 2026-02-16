<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState } from '../composables/planState';
import { flattenRelOps } from '../composables/sqlPlanParser';
import { getOperatorIcon, formatTime } from '../types/sqlplan';
import type { RelOp } from '../types/sqlplan';

const { 
  state, 
  selectComparisonStatement,
  clearComparisonPlan,
  comparisonStatements,
} = usePlanState();

// Computed: comparison metrics
const primaryMetrics = computed(() => {
  if (!state.selectedStatement) return null;
  const nodes = flattenRelOps(state.selectedStatement.queryPlan.relOp);
  const totalCost = nodes.reduce((sum, n) => sum + n.estimatedTotalSubtreeCost, 0);
  const hasRuntime = nodes.some(n => n.runtimeInfo);
  const totalTime = hasRuntime 
    ? nodes.reduce((sum, n) => sum + (n.runtimeInfo?.actualElapsedMs || 0), 0)
    : null;
  const totalRows = hasRuntime
    ? nodes.reduce((sum, n) => sum + (n.runtimeInfo?.actualRows || 0), 0)
    : null;
  
  return {
    nodeCount: nodes.length,
    totalCost,
    totalTime,
    totalRows,
    operators: getOperatorBreakdown(nodes),
    statement: state.selectedStatement,
  };
});

const comparisonMetrics = computed(() => {
  if (!state.comparisonStatement) return null;
  const nodes = flattenRelOps(state.comparisonStatement.queryPlan.relOp);
  const totalCost = nodes.reduce((sum, n) => sum + n.estimatedTotalSubtreeCost, 0);
  const hasRuntime = nodes.some(n => n.runtimeInfo);
  const totalTime = hasRuntime 
    ? nodes.reduce((sum, n) => sum + (n.runtimeInfo?.actualElapsedMs || 0), 0)
    : null;
  const totalRows = hasRuntime
    ? nodes.reduce((sum, n) => sum + (n.runtimeInfo?.actualRows || 0), 0)
    : null;
  
  return {
    nodeCount: nodes.length,
    totalCost,
    totalTime,
    totalRows,
    operators: getOperatorBreakdown(nodes),
    statement: state.comparisonStatement,
  };
});

// Get breakdown of operators used
const getOperatorBreakdown = (nodes: RelOp[]): Map<string, number> => {
  const breakdown = new Map<string, number>();
  for (const node of nodes) {
    const op = node.physicalOp || node.logicalOp;
    breakdown.set(op, (breakdown.get(op) || 0) + 1);
  }
  return breakdown;
};

// Calculate improvement/regression percentage
const getCostDiff = computed(() => {
  if (!primaryMetrics.value || !comparisonMetrics.value) return null;
  const primary = primaryMetrics.value.totalCost;
  const comparison = comparisonMetrics.value.totalCost;
  if (primary === 0) return null;
  return ((comparison - primary) / primary) * 100;
});

const getTimeDiff = computed(() => {
  if (!primaryMetrics.value?.totalTime || !comparisonMetrics.value?.totalTime) return null;
  const primary = primaryMetrics.value.totalTime;
  const comparison = comparisonMetrics.value.totalTime;
  if (primary === 0) return null;
  return ((comparison - primary) / primary) * 100;
});

// Format percentage with color indicator
const formatDiff = (diff: number | null): { text: string; class: string } => {
  if (diff === null) return { text: 'N/A', class: 'text-slate-400' };
  const sign = diff > 0 ? '+' : '';
  if (Math.abs(diff) < 1) {
    return { text: '≈ Same', class: 'text-slate-300' };
  }
  if (diff > 0) {
    return { text: `${sign}${diff.toFixed(1)}%`, class: 'text-red-400' }; // Worse
  }
  return { text: `${diff.toFixed(1)}%`, class: 'text-green-400' }; // Better
};

// Find operators that differ between plans
const operatorDiffs = computed(() => {
  if (!primaryMetrics.value || !comparisonMetrics.value) return [];
  
  const primary = primaryMetrics.value.operators;
  const comparison = comparisonMetrics.value.operators;
  const allOps = new Set([...primary.keys(), ...comparison.keys()]);
  
  const diffs: Array<{ op: string; primary: number; comparison: number; diff: 'added' | 'removed' | 'changed' | 'same' }> = [];
  
  for (const op of allOps) {
    const pCount = primary.get(op) || 0;
    const cCount = comparison.get(op) || 0;
    
    let diff: 'added' | 'removed' | 'changed' | 'same' = 'same';
    if (pCount === 0) diff = 'added';
    else if (cCount === 0) diff = 'removed';
    else if (pCount !== cCount) diff = 'changed';
    
    if (diff !== 'same') {
      diffs.push({ op, primary: pCount, comparison: cCount, diff });
    }
  }
  
  return diffs;
});
</script>

<template>
  <div class="h-full flex flex-col bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 bg-slate-700 border-b border-slate-600">
      <div class="flex items-center gap-2">
        <i class="fa-solid fa-code-compare text-blue-400"></i>
        <span class="font-medium text-slate-200">Plan Comparison</span>
      </div>
      <button 
        class="text-slate-400 hover:text-slate-200 transition-colors"
        title="Close Comparison"
        @click="clearComparisonPlan"
      >
        <i class="fa-solid fa-xmark"></i>
      </button>
    </div>
    
    <!-- Content -->
    <div class="flex-1 overflow-auto p-4 space-y-6">
      <!-- Statement Selectors -->
      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="block text-xs text-slate-400 mb-1">Primary Plan</label>
          <div class="text-sm text-slate-200 bg-slate-900 rounded px-3 py-2 truncate">
            {{ state.selectedStatement?.statementText || 'No statement selected' }}
          </div>
        </div>
        <div>
          <label class="block text-xs text-slate-400 mb-1">Comparison Plan</label>
          <select
            v-if="comparisonStatements.length > 0"
            class="w-full bg-slate-900 text-slate-200 rounded px-3 py-2 text-sm border border-slate-700 focus:border-blue-500 focus:outline-none"
            :value="state.comparisonStatement?.statementId || ''"
            @change="selectComparisonStatement(comparisonStatements.find(s => s.statementId === Number(($event.target as HTMLSelectElement).value)) || null)"
          >
            <option 
              v-for="stmt in comparisonStatements" 
              :key="stmt.statementId" 
              :value="stmt.statementId"
            >
              {{ stmt.statementText?.substring(0, 50) || 'Statement' }}...
            </option>
          </select>
          <div v-else class="text-sm text-slate-400 bg-slate-900 rounded px-3 py-2">
            Load a comparison plan
          </div>
        </div>
      </div>
      
      <!-- Summary Comparison -->
      <div v-if="primaryMetrics && comparisonMetrics" class="space-y-4">
        <!-- Overall Metrics -->
        <div class="bg-slate-900 rounded-lg p-4">
          <h3 class="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
            <i class="fa-solid fa-chart-bar text-blue-400"></i>
            Overall Comparison
          </h3>
          
          <div class="flex flex-wrap justify-center gap-x-8 gap-y-4">
            <!-- Cost Comparison -->
            <div class="text-center min-w-35">
              <div class="text-xs text-slate-400 mb-1">Estimated Cost</div>
              <div class="flex items-center justify-center gap-1 mb-1 text-sm whitespace-nowrap">
                <span class="text-slate-200">{{ primaryMetrics.totalCost.toFixed(2) }}</span>
                <i class="fa-solid fa-arrow-right text-slate-500 text-xs"></i>
                <span class="text-slate-200">{{ comparisonMetrics.totalCost.toFixed(2) }}</span>
              </div>
              <div :class="formatDiff(getCostDiff).class" class="text-sm font-medium">
                {{ formatDiff(getCostDiff).text }}
              </div>
            </div>

            <!-- Time Comparison -->
            <div class="text-center min-w-35">
              <div class="text-xs text-slate-400 mb-1">Actual Time</div>
              <div class="flex items-center justify-center gap-1 mb-1 text-sm whitespace-nowrap">
                <span class="text-slate-200">
                  {{ primaryMetrics.totalTime !== null ? formatTime(primaryMetrics.totalTime) : 'N/A' }}
                </span>
                <i class="fa-solid fa-arrow-right text-slate-500 text-xs"></i>
                <span class="text-slate-200">
                  {{ comparisonMetrics.totalTime !== null ? formatTime(comparisonMetrics.totalTime) : 'N/A' }}
                </span>
              </div>
              <div :class="formatDiff(getTimeDiff).class" class="text-sm font-medium">
                {{ formatDiff(getTimeDiff).text }}
              </div>
            </div>

            <!-- Node Count -->
            <div class="text-center min-w-35">
              <div class="text-xs text-slate-400 mb-1">Operators</div>
              <div class="flex items-center justify-center gap-1 mb-1 text-sm whitespace-nowrap">
                <span class="text-slate-200">{{ primaryMetrics.nodeCount }}</span>
                <i class="fa-solid fa-arrow-right text-slate-500 text-xs"></i>
                <span class="text-slate-200">{{ comparisonMetrics.nodeCount }}</span>
              </div>
              <div class="text-sm text-slate-400">
                {{ comparisonMetrics.nodeCount - primaryMetrics.nodeCount > 0 ? '+' : '' }}{{ comparisonMetrics.nodeCount - primaryMetrics.nodeCount }}
              </div>
            </div>
          </div>
        </div>
        
        <!-- Operator Differences -->
        <div v-if="operatorDiffs.length > 0" class="bg-slate-900 rounded-lg p-4">
          <h3 class="text-sm font-medium text-slate-300 mb-3 flex items-center gap-2">
            <i class="fa-solid fa-shuffle text-amber-400"></i>
            Operator Changes
          </h3>
          
          <div class="space-y-2">
            <div 
              v-for="diff in operatorDiffs" 
              :key="diff.op"
              class="flex items-center justify-between text-sm"
            >
              <div class="flex items-center gap-2">
                <i :class="[getOperatorIcon(diff.op), {
                  'text-green-400': diff.diff === 'removed',
                  'text-red-400': diff.diff === 'added',
                  'text-amber-400': diff.diff === 'changed',
                }]"></i>
                <span class="text-slate-200">{{ diff.op }}</span>
              </div>
              <div class="flex items-center gap-2 text-slate-400">
                <span :class="{ 'line-through': diff.diff === 'added' }">{{ diff.primary }}</span>
                <i class="fa-solid fa-arrow-right text-xs"></i>
                <span :class="{ 
                  'text-green-400': diff.diff === 'removed',
                  'text-red-400': diff.diff === 'added',
                  'text-amber-400': diff.diff === 'changed',
                }">{{ diff.comparison }}</span>
              </div>
            </div>
          </div>
        </div>
        
        <!-- Legend -->
        <div class="flex items-center justify-center gap-6 text-xs text-slate-400">
          <div class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-green-400"></span>
            <span>Improvement</span>
          </div>
          <div class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-red-400"></span>
            <span>Regression</span>
          </div>
          <div class="flex items-center gap-1">
            <span class="w-2 h-2 rounded-full bg-amber-400"></span>
            <span>Changed</span>
          </div>
        </div>
      </div>
      
      <!-- No comparison loaded -->
      <div v-else class="flex flex-col items-center justify-center py-12 text-slate-400">
        <i class="fa-solid fa-file-circle-plus text-4xl mb-3"></i>
        <p class="text-sm text-center">
          Load a second .sqlplan file to compare<br/>execution plans side by side
        </p>
      </div>
    </div>
  </div>
</template>
