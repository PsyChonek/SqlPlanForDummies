<script setup lang="ts">
import { computed } from 'vue';
import { usePlanState } from '../composables/planState';
import { flattenRelOps } from '../composables/sqlPlanParser';
import { formatTime } from '../types/sqlplan';
import { getWaitTypeDescription } from '../types/waitTypes';
import CollapsiblePanel from './CollapsiblePanel.vue';
import type { Statement, PlanWarnings, WaitStat } from '../types/sqlplan';

const emit = defineEmits<{ (e: 'statement-selected'): void }>();

const { state, statements, selectStatement } = usePlanState();

const statementElapsedMs = (stmt: Statement): number | null =>
  stmt.queryPlan.queryTimeStats?.elapsedTimeMs
  ?? stmt.queryPlan.relOp.runtimeInfo?.actualElapsedMs
  ?? null;

const statementCpuMs = (stmt: Statement): number | null =>
  stmt.queryPlan.queryTimeStats?.cpuTimeMs ?? null;

const statementWaitMs = (stmt: Statement): number =>
  (stmt.queryPlan.waitStats ?? []).reduce((sum, w) => sum + w.waitTimeMs, 0);

const hasLockWaits = (stmt: Statement): boolean =>
  stmt.queryPlan.waitStats?.some(w => w.waitType.startsWith('LCK_M_')) ?? false;

// Turn a Warnings element into short human-readable labels
function warningLabels(warnings: PlanWarnings | undefined, source: string): string[] {
  if (!warnings) return [];
  const labels: string[] = [];
  if (warnings.noJoinPredicate) labels.push(`${source}: No join predicate (cartesian product)`);
  if (warnings.unmatchedIndexes) labels.push(`${source}: Filtered index not usable due to parameterization`);
  for (const spill of warnings.spills ?? []) {
    labels.push(`${source}: ${spill.kind} spill to tempdb${spill.spillLevel ? ` (level ${spill.spillLevel})` : ''}`);
  }
  for (const conv of warnings.planAffectingConverts ?? []) {
    labels.push(`${source}: Type conversion affecting ${conv.convertIssue.toLowerCase()}`);
  }
  if (warnings.columnsWithNoStatistics?.length) {
    labels.push(`${source}: Missing statistics on ${warnings.columnsWithNoStatistics.map(c => c.column).join(', ')}`);
  }
  for (const mg of warnings.memoryGrantWarnings ?? []) {
    labels.push(`${source}: Memory grant warning (${mg.kind})`);
  }
  return labels;
}

// All warnings of a statement: statement-level plus per-operator
function statementWarnings(stmt: Statement): string[] {
  const labels = warningLabels(stmt.queryPlan.warnings, 'Statement');
  for (const node of flattenRelOps(stmt.queryPlan.relOp)) {
    labels.push(...warningLabels(node.warnings, node.physicalOp));
  }
  return labels;
}

interface StatementRow {
  stmt: Statement;
  index: number;
  elapsedMs: number | null;
  cpuMs: number | null;
  waitMs: number;
  lockWaits: boolean;
  warnings: string[];
  missingIndexCount: number;
}

const rows = computed((): StatementRow[] =>
  statements.value.map((stmt, index) => ({
    stmt,
    index,
    elapsedMs: statementElapsedMs(stmt),
    cpuMs: statementCpuMs(stmt),
    waitMs: statementWaitMs(stmt),
    lockWaits: hasLockWaits(stmt),
    warnings: statementWarnings(stmt),
    missingIndexCount: stmt.queryPlan.missingIndexes?.length ?? 0,
  }))
);

// Plan-wide totals
const totals = computed(() => {
  const r = rows.value;
  const sum = (vals: Array<number | null>) => {
    const present = vals.filter((v): v is number => v !== null);
    return present.length > 0 ? present.reduce((a, b) => a + b, 0) : null;
  };
  return {
    statements: r.length,
    totalCost: r.reduce((a, b) => a + b.stmt.statementSubTreeCost, 0),
    elapsedMs: sum(r.map(x => x.elapsedMs)),
    cpuMs: sum(r.map(x => x.cpuMs)),
    waitMs: r.reduce((a, b) => a + b.waitMs, 0),
    warningCount: r.reduce((a, b) => a + b.warnings.length, 0),
    missingIndexCount: r.reduce((a, b) => a + b.missingIndexCount, 0),
    lockWaits: r.some(x => x.lockWaits),
  };
});

interface AggregatedWait extends WaitStat {
  statementIndexes: number[];
}

// Waits merged across all statements, sorted by total time
const aggregatedWaits = computed((): AggregatedWait[] => {
  const byType = new Map<string, AggregatedWait>();
  for (const row of rows.value) {
    for (const wait of row.stmt.queryPlan.waitStats ?? []) {
      const entry = byType.get(wait.waitType);
      if (entry) {
        entry.waitTimeMs += wait.waitTimeMs;
        entry.waitCount += wait.waitCount;
        entry.statementIndexes.push(row.index);
      } else {
        byType.set(wait.waitType, { ...wait, statementIndexes: [row.index] });
      }
    }
  }
  return [...byType.values()].sort((a, b) => b.waitTimeMs - a.waitTimeMs);
});

const totalWaitMs = computed(() => totals.value.waitMs);

const isLockWait = (waitType: string) => waitType.startsWith('LCK_M_');

// Warnings grouped per statement, only statements that have any
const warningRows = computed(() => rows.value.filter(r => r.warnings.length > 0));

// Missing indexes with their statement index
const missingIndexRows = computed(() =>
  rows.value.flatMap(r =>
    (r.stmt.queryPlan.missingIndexes ?? []).map(mi => ({ mi, index: r.index }))
  ).sort((a, b) => b.mi.impact - a.mi.impact)
);

const goToStatement = (stmt: Statement) => {
  selectStatement(stmt);
  emit('statement-selected');
};
</script>

<template>
  <div class="h-full overflow-y-auto p-4 space-y-4">
    <!-- Empty state -->
    <div v-if="!state.plan" class="h-full flex flex-col items-center justify-center text-slate-500">
      <i class="fa-solid fa-clipboard-list text-5xl mb-4"></i>
      <p class="text-sm">Load a plan to see the overview</p>
      <p class="text-xs text-slate-600 mt-1">You can paste a plan from the clipboard in the XML tab</p>
    </div>

    <template v-else>
      <!-- Totals -->
      <div class="grid grid-cols-3 xl:grid-cols-6 gap-2">
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold text-white">{{ totals.statements }}</div>
          <div class="text-xs text-slate-400">Statements</div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold text-white">{{ totals.totalCost.toFixed(3) }}</div>
          <div class="text-xs text-slate-400">Total Cost</div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold text-white">{{ totals.elapsedMs !== null ? formatTime(totals.elapsedMs) : '-' }}</div>
          <div class="text-xs text-slate-400">Elapsed</div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold text-white">{{ totals.cpuMs !== null ? formatTime(totals.cpuMs) : '-' }}</div>
          <div class="text-xs text-slate-400">CPU</div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold" :class="totals.lockWaits ? 'text-red-400' : totals.waitMs > 0 ? 'text-amber-400' : 'text-slate-300'">
            {{ totals.waitMs > 0 ? formatTime(totals.waitMs) : '-' }}
          </div>
          <div class="text-xs text-slate-400" title="Summed across all parallel worker threads - can exceed elapsed time">
            Waits (all threads)
          </div>
        </div>
        <div class="bg-slate-700/50 rounded-lg p-3 text-center">
          <div class="text-2xl font-bold" :class="totals.warningCount > 0 ? 'text-amber-400' : 'text-slate-300'">
            {{ totals.warningCount }}
          </div>
          <div class="text-xs text-slate-400">Warnings</div>
        </div>
      </div>

      <!-- Statements table -->
      <CollapsiblePanel title="Statements" icon="fa-list-ol" icon-color="text-blue-400" :badge="rows.length">
        <div class="overflow-x-auto">
          <table class="w-full text-xs">
            <thead>
              <tr class="text-slate-400 border-b border-slate-600">
                <th class="text-left py-2 pr-2 font-semibold">#</th>
                <th class="text-left py-2 pr-2 font-semibold">Statement</th>
                <th class="text-right py-2 pr-2 font-semibold">Cost</th>
                <th class="text-right py-2 pr-2 font-semibold">Elapsed</th>
                <th class="text-right py-2 pr-2 font-semibold">CPU</th>
                <th class="text-right py-2 pr-2 font-semibold">Waits</th>
                <th class="text-right py-2 font-semibold">Warnings</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row in rows"
                :key="row.stmt.statementId"
                class="border-b border-slate-700/50 cursor-pointer transition-colors"
                :class="state.selectedStatement?.statementId === row.stmt.statementId
                  ? 'bg-blue-600/20'
                  : 'hover:bg-slate-700/50'"
                @click="goToStatement(row.stmt)"
              >
                <td class="py-2 pr-2 text-slate-400">{{ row.index + 1 }}</td>
                <td class="py-2 pr-2 max-w-0 w-full">
                  <div class="truncate text-slate-200 font-mono" :title="row.stmt.statementText">
                    {{ row.stmt.statementType }}: {{ row.stmt.statementText }}
                  </div>
                </td>
                <td class="py-2 pr-2 text-right font-mono text-slate-300">{{ row.stmt.statementSubTreeCost.toFixed(4) }}</td>
                <td class="py-2 pr-2 text-right font-mono text-slate-300 whitespace-nowrap">
                  {{ row.elapsedMs !== null ? formatTime(row.elapsedMs) : '-' }}
                </td>
                <td class="py-2 pr-2 text-right font-mono text-slate-300 whitespace-nowrap">
                  {{ row.cpuMs !== null ? formatTime(row.cpuMs) : '-' }}
                </td>
                <td class="py-2 pr-2 text-right font-mono whitespace-nowrap" :class="row.lockWaits ? 'text-red-400' : row.waitMs > 0 ? 'text-amber-400' : 'text-slate-500'">
                  <i v-if="row.lockWaits" class="fa-solid fa-lock mr-1"></i>
                  {{ row.waitMs > 0 ? formatTime(row.waitMs) : '-' }}
                </td>
                <td class="py-2 text-right" :class="row.warnings.length > 0 ? 'text-amber-400 font-semibold' : 'text-slate-500'">
                  {{ row.warnings.length > 0 ? row.warnings.length : '-' }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </CollapsiblePanel>

      <!-- Aggregated waits -->
      <CollapsiblePanel v-if="aggregatedWaits.length > 0" title="Wait Statistics (all statements)" icon="fa-hourglass-half" icon-color="text-amber-400" :badge="aggregatedWaits.length">
        <p
          v-if="totals.elapsedMs !== null && totals.waitMs > totals.elapsedMs"
          class="text-xs text-slate-400 bg-blue-500/10 border border-blue-500/30 rounded-lg px-3 py-2 mb-2"
        >
          <i class="fa-solid fa-circle-info text-blue-400 mr-1"></i>
          Wait times are summed across all parallel worker threads of every statement,
          so they can exceed the total elapsed time of {{ formatTime(totals.elapsedMs) }}.
          One second of waiting on 8 threads counts as 8 seconds here.
        </p>
        <div class="space-y-2">
          <div
            v-for="wait in aggregatedWaits"
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
              <span>
                {{ wait.waitCount.toLocaleString() }} wait{{ wait.waitCount !== 1 ? 's' : '' }}
                in statement{{ wait.statementIndexes.length !== 1 ? 's' : '' }} {{ wait.statementIndexes.map(i => i + 1).join(', ') }}
              </span>
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

      <!-- Warnings per statement -->
      <CollapsiblePanel v-if="warningRows.length > 0" title="Warnings" icon="fa-triangle-exclamation" icon-color="text-amber-400" :badge="totals.warningCount">
        <div class="space-y-3">
          <div v-for="row in warningRows" :key="row.stmt.statementId">
            <button
              class="text-xs font-semibold text-blue-400 hover:text-blue-300 mb-1"
              @click="goToStatement(row.stmt)"
            >
              Statement {{ row.index + 1 }}
              <span class="text-slate-500 font-normal font-mono">{{ row.stmt.statementText.substring(0, 60) }}...</span>
            </button>
            <ul class="space-y-1">
              <li
                v-for="(warning, wIdx) in row.warnings"
                :key="wIdx"
                class="text-xs text-slate-300 bg-amber-500/10 border border-amber-500/30 rounded px-2 py-1"
              >
                <i class="fa-solid fa-triangle-exclamation text-amber-400 mr-1"></i>
                {{ warning }}
              </li>
            </ul>
          </div>
        </div>
      </CollapsiblePanel>

      <!-- Missing indexes -->
      <CollapsiblePanel v-if="missingIndexRows.length > 0" title="Missing Indexes" icon="fa-database" icon-color="text-emerald-400" :badge="missingIndexRows.length">
        <div class="space-y-2">
          <div
            v-for="({ mi, index }, miIdx) in missingIndexRows"
            :key="miIdx"
            class="bg-slate-700/50 rounded-lg p-3"
          >
            <div class="flex items-center justify-between mb-1">
              <span class="text-sm font-semibold text-slate-200">{{ mi.table }}</span>
              <span class="text-xs font-bold" :class="mi.impact >= 80 ? 'text-red-400' : 'text-amber-400'">
                {{ mi.impact.toFixed(0) }}% impact
              </span>
            </div>
            <p class="text-xs font-mono text-slate-400">
              ({{ [...mi.equalityColumns, ...mi.inequalityColumns].join(', ') }})<span v-if="mi.includeColumns.length"> INCLUDE ({{ mi.includeColumns.join(', ') }})</span>
            </p>
            <p class="text-xs text-slate-500 mt-1">Statement {{ index + 1 }}</p>
          </div>
        </div>
      </CollapsiblePanel>
    </template>
  </div>
</template>
