<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState } from '../composables/planState';
import { flattenRelOps } from '../composables/sqlPlanParser';
import CollapsiblePanel from './CollapsiblePanel.vue';
import { formatTime } from '../types/sqlplan';
import { getWaitTypeDescription } from '../types/waitTypes';
import type { RelOp, PlanWarnings } from '../types/sqlplan';

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
    
    // Row estimate vs actual mismatch.
    // EstimateRows is per execution, ActualRows is a total — compare against
    // estimate * executions. Only report mismatches large enough (in both ratio
    // and absolute rows) to actually affect plan choice; "estimated 2, got 0" is noise.
    if (node.runtimeInfo) {
      const executions = Math.max(1, node.runtimeInfo.actualExecutions);
      const expectedRows = node.estimateRows * executions;
      const actualRows = node.runtimeInfo.actualRows;
      const underestimated = actualRows > expectedRows;
      const factor = underestimated
        ? actualRows / Math.max(1, expectedRows)
        : expectedRows / Math.max(1, actualRows);

      if (factor >= 10 && Math.max(actualRows, expectedRows) >= 1000) {
        results.push({
          severity: underestimated ? (factor >= 100 ? 'critical' : 'warning') : 'info',
          title: 'Estimate Mismatch',
          description: underestimated
            ? `${node.physicalOp}: Estimated ${Math.round(expectedRows).toLocaleString()} rows but got ${actualRows.toLocaleString()} (${Math.round(factor)}x more). Statistics may be outdated or the predicate is hard to estimate.`
            : `${node.physicalOp}: Estimated ${Math.round(expectedRows).toLocaleString()} rows but got ${actualRows.toLocaleString()} (${Math.round(factor)}x fewer). The plan may reserve more memory than needed.`,
          nodeId: node.nodeId,
          impact: Math.min(100, Math.log10(factor) * (underestimated ? 30 : 15)),
        });
      }
    }

    // Operator-level warnings emitted by SQL Server itself
    if (node.warnings) {
      results.push(...warningsToIssues(node.warnings, costPct, node.nodeId, node.physicalOp));
    }
  }

  // Statement-level warnings and missing index suggestions
  const plan = state.selectedStatement.queryPlan;
  if (plan.warnings) {
    results.push(...warningsToIssues(plan.warnings, 50, undefined, 'Statement'));
  }
  for (const mi of plan.missingIndexes ?? []) {
    const keyColumns = [...mi.equalityColumns, ...mi.inequalityColumns].join(', ');
    const include = mi.includeColumns.length > 0 ? ` INCLUDE (${mi.includeColumns.join(', ')})` : '';
    results.push({
      severity: mi.impact >= 80 ? 'critical' : 'warning',
      title: 'Missing Index Suggested',
      description: `Optimizer suggests an index on ${mi.table} (${keyColumns})${include}. Estimated improvement: ${mi.impact.toFixed(0)}%.`,
      impact: Math.min(100, mi.impact),
    });
  }

  // Statement-level lock waits — the query spent time blocked on locks,
  // which is invisible in operator costs but dominates real elapsed time
  const lockWaitMs = (state.selectedStatement.queryPlan.waitStats ?? [])
    .filter(w => w.waitType.startsWith('LCK_M_'))
    .reduce((sum, w) => sum + w.waitTimeMs, 0);
  if (lockWaitMs > 0) {
    const elapsed = state.selectedStatement.queryPlan.queryTimeStats?.elapsedTimeMs;
    const pctOfElapsed = elapsed ? (lockWaitMs / elapsed) * 100 : null;
    results.push({
      severity: pctOfElapsed === null || pctOfElapsed > 25 ? 'critical' : 'warning',
      title: 'Blocking: Lock Waits',
      description: `Query waited ${formatTime(lockWaitMs)} on locks`
        + (pctOfElapsed !== null ? ` (${pctOfElapsed.toFixed(0)}% of elapsed time)` : '')
        + '. Another session was blocking this query - the plan itself may be fine.',
      impact: pctOfElapsed !== null ? Math.min(100, pctOfElapsed) : 100,
    });
  }

  // Sort by impact
  return results.sort((a, b) => b.impact - a.impact).slice(0, 10);
});

// Convert a parsed Warnings element (operator- or statement-level) into issues
function warningsToIssues(warnings: PlanWarnings, costPct: number, nodeId: number | undefined, opName: string): Issue[] {
  const results: Issue[] = [];

  for (const spill of warnings.spills ?? []) {
    const detail = spill.writesToTempDb
      ? ` (${spill.writesToTempDb.toLocaleString()} pages written to tempdb)`
      : spill.spillLevel ? ` (spill level ${spill.spillLevel})` : '';
    results.push({
      severity: 'critical',
      title: 'Spill to TempDb',
      description: `${opName} ran out of granted memory and spilled to tempdb${detail}. Usually caused by row underestimates; check statistics and memory grants.`,
      nodeId,
      impact: Math.max(60, Math.min(100, costPct * 2)),
    });
  }

  if (warnings.noJoinPredicate) {
    results.push({
      severity: 'critical',
      title: 'No Join Predicate',
      description: 'A join has no predicate, producing a cartesian product. Check for a missing JOIN condition.',
      nodeId,
      impact: 90,
    });
  }

  for (const convert of warnings.planAffectingConverts ?? []) {
    const consequence = convert.convertIssue === 'Seek Plan'
      ? 'prevents an index seek'
      : 'affects cardinality estimates';
    results.push({
      severity: 'warning',
      title: 'Implicit Conversion',
      description: `Type conversion in ${convert.expression} ${consequence}. Check for data type mismatches (e.g. NVARCHAR parameter vs VARCHAR column).`,
      nodeId,
      impact: convert.convertIssue === 'Seek Plan' ? 70 : 40,
    });
  }

  for (const grant of warnings.memoryGrantWarnings ?? []) {
    results.push({
      severity: 'warning',
      title: 'Memory Grant Warning',
      description: `Memory grant issue (${grant.kind})` +
        (grant.grantedMemoryKb ? `: granted ${grant.grantedMemoryKb.toLocaleString()} KB` : '') +
        (grant.usedMemoryKb ? `, used ${grant.usedMemoryKb.toLocaleString()} KB` : '') +
        '. The query may have waited for memory or reserved far more than needed.',
      nodeId,
      impact: 50,
    });
  }

  if ((warnings.columnsWithNoStatistics ?? []).length > 0) {
    const cols = (warnings.columnsWithNoStatistics ?? []).map(c => c.column).join(', ');
    results.push({
      severity: 'info',
      title: 'Columns Without Statistics',
      description: `No statistics available for: ${cols}. The optimizer is guessing row counts for these columns.`,
      nodeId,
      impact: 30,
    });
  }

  return results;
}

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
  
  // Calculate total actual time if available.
  // Prefer statement-level QueryTimeStats: it includes time spent waiting
  // (e.g. blocked on locks), which operator elapsed times can understate.
  const timeStats = state.selectedStatement.queryPlan.queryTimeStats;
  let totalActualTime = 0;
  let hasRuntimeInfo = false;
  if (timeStats) {
    hasRuntimeInfo = true;
    totalActualTime = timeStats.elapsedTimeMs;
  } else {
    for (const node of nodes) {
      if (node.runtimeInfo) {
        hasRuntimeInfo = true;
        totalActualTime = Math.max(totalActualTime, node.runtimeInfo.actualElapsedMs);
      }
    }
  }

  return {
    totalNodes,
    parallelNodes,
    scanNodes,
    seekNodes,
    totalCost: state.selectedStatement.statementSubTreeCost,
    actualTime: hasRuntimeInfo ? totalActualTime : null,
    cpuTime: timeStats ? timeStats.cpuTimeMs : null,
  };
});

// Statement-level wait statistics (from QueryPlan/WaitStats in actual plans)
const waitStats = computed(() => state.selectedStatement?.queryPlan.waitStats ?? []);

const totalWaitMs = computed(() =>
  waitStats.value.reduce((sum, w) => sum + w.waitTimeMs, 0)
);

const isLockWait = (waitType: string) => waitType.startsWith('LCK_M_');

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
      <CollapsiblePanel v-if="planStats" title="Plan Statistics" icon="fa-chart-pie" icon-color="text-cyan-400">
        <div class="grid grid-cols-3 gap-2">
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
        <div v-if="planStats?.actualTime !== null && planStats?.actualTime !== undefined" class="bg-slate-700/50 rounded-lg p-3 mt-2 space-y-2">
          <div class="flex items-center justify-between">
            <span class="text-sm text-slate-400">
              <i class="fa-solid fa-stopwatch mr-1"></i>
              Actual Elapsed Time
            </span>
            <span class="font-mono font-bold text-white">
              {{ formatTime(planStats.actualTime) }}
            </span>
          </div>
          <div v-if="planStats?.cpuTime !== null && planStats?.cpuTime !== undefined" class="flex items-center justify-between">
            <span class="text-sm text-slate-400">
              <i class="fa-solid fa-microchip mr-1"></i>
              CPU Time
            </span>
            <span class="font-mono font-bold text-white">
              {{ formatTime(planStats.cpuTime) }}
            </span>
          </div>
          <div v-if="totalWaitMs > 0" class="flex items-center justify-between">
            <span class="text-sm text-slate-400">
              <i class="fa-solid fa-hourglass-half mr-1"></i>
              Total Wait Time
            </span>
            <span class="font-mono font-bold text-amber-400">
              {{ formatTime(totalWaitMs) }}
            </span>
          </div>
        </div>
      </CollapsiblePanel>

      <!-- Wait Statistics -->
      <CollapsiblePanel v-if="waitStats.length > 0" title="Wait Statistics" icon="fa-hourglass-half" icon-color="text-amber-400" :badge="waitStats.length">
        <p
          v-if="planStats?.actualTime != null && totalWaitMs > planStats.actualTime"
          class="text-xs text-slate-400 bg-blue-500/10 border border-blue-500/30 rounded-lg px-3 py-2 mb-2"
        >
          <i class="fa-solid fa-circle-info text-blue-400 mr-1"></i>
          Wait times are summed across all parallel worker threads
          (DOP {{ state.selectedStatement?.queryPlan.degreeOfParallelism || 1 }}),
          so they can exceed the query's elapsed time of {{ formatTime(planStats.actualTime) }}.
          One second of waiting on 8 threads counts as 8 seconds here.
        </p>
        <div class="space-y-2">
          <div
            v-for="wait in waitStats"
            :key="wait.waitType"
            class="bg-slate-700/50 rounded-lg p-3"
          >
            <div class="flex items-center justify-between mb-1">
              <span class="font-mono text-sm font-semibold" :class="isLockWait(wait.waitType) ? 'text-red-400' : 'text-slate-200'">
                <i v-if="isLockWait(wait.waitType)" class="fa-solid fa-lock mr-1"></i>
                {{ wait.waitType }}
              </span>
              <span class="font-mono text-sm font-bold text-white">{{ formatTime(wait.waitTimeMs) }}</span>
            </div>
            <div class="flex justify-between text-xs text-slate-500 mb-1">
              <span>{{ wait.waitCount.toLocaleString() }} wait{{ wait.waitCount !== 1 ? 's' : '' }}</span>
              <span>{{ totalWaitMs > 0 ? ((wait.waitTimeMs / totalWaitMs) * 100).toFixed(1) : 0 }}%</span>
            </div>
            <p v-if="getWaitTypeDescription(wait.waitType)" class="text-xs text-slate-400 mb-1">
              {{ getWaitTypeDescription(wait.waitType) }}
            </p>
            <div class="h-1 bg-slate-600 rounded-full overflow-hidden">
              <div
                class="h-full rounded-full"
                :class="isLockWait(wait.waitType) ? 'bg-red-500' : 'bg-amber-500'"
                :style="{ width: (totalWaitMs > 0 ? (wait.waitTimeMs / totalWaitMs) * 100 : 0) + '%' }"
              ></div>
            </div>
          </div>
        </div>
      </CollapsiblePanel>

      <!-- Issues List -->
      <CollapsiblePanel v-if="issues.length > 0" title="Detected Issues" icon="fa-list-check" icon-color="text-amber-400" :badge="issues.length">
        <div class="space-y-2">
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
      </CollapsiblePanel>

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
