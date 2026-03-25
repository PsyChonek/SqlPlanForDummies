<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import {
  getEventSeverity, getEventSeverityColor, getEventIcon,
  formatDuration, formatNumber, formatTimestampFull,
  getLockModeDescription,
} from '../../types/xel';
import type { BlockingAnalysis, TransactionObject, XelEvent } from '../../types/xel';

const { state, selectEvent, setFilter, clearFilter } = useXelState();

const copied = ref('');
const copyText = async (text: string, label: string) => {
  await navigator.clipboard.writeText(text);
  copied.value = label;
  setTimeout(() => { copied.value = ''; }, 1500);
};

const buildLlmPrompt = (): string => {
  const ev = state.selectedEvent;
  if (!ev) return '';

  const lines: string[] = [];
  lines.push('# SQL Server Extended Events - Event Analysis');
  lines.push('');
  lines.push('## Event');
  lines.push(`- **Event Name:** ${ev.eventName}`);
  lines.push(`- **Timestamp:** ${ev.timestamp}`);
  lines.push(`- **Event ID:** ${ev.id}`);
  if (ev.sessionId !== null) lines.push(`- **Session ID:** ${ev.sessionId}`);
  if (ev.databaseName) lines.push(`- **Database:** ${ev.databaseName}`);
  if (ev.username) lines.push(`- **User:** ${ev.username}`);
  if (ev.clientAppName) lines.push(`- **Application:** ${ev.clientAppName}`);
  if (ev.objectName) lines.push(`- **Object:** ${ev.objectName}`);
  if (ev.result) lines.push(`- **Result:** ${ev.result}`);
  lines.push(`- **Source File:** ${ev.sourceFile}`);

  // Performance
  if (ev.durationUs !== null || ev.logicalReads !== null) {
    lines.push('');
    lines.push('## Performance');
    if (ev.durationUs !== null) lines.push(`- **Duration:** ${formatDuration(ev.durationUs)} (${ev.durationUs} µs)`);
    if (ev.cpuTimeUs !== null) lines.push(`- **CPU Time:** ${formatDuration(ev.cpuTimeUs)} (${ev.cpuTimeUs} µs)`);
    if (ev.durationUs !== null && ev.cpuTimeUs !== null && ev.durationUs > ev.cpuTimeUs * 2) {
      lines.push(`- **Wait Ratio:** ${Math.round((1 - ev.cpuTimeUs / ev.durationUs) * 100)}% waiting`);
    }
    if (ev.logicalReads !== null) lines.push(`- **Logical Reads:** ${ev.logicalReads.toLocaleString()}`);
    if (ev.physicalReads !== null) lines.push(`- **Physical Reads:** ${ev.physicalReads.toLocaleString()}`);
    if (ev.writes !== null) lines.push(`- **Writes:** ${ev.writes.toLocaleString()}`);
  }

  // Lock / Wait
  if (ev.resourceType || ev.lockMode || ev.waitType || ev.waitDurationMs !== null) {
    lines.push('');
    lines.push('## Lock / Wait Information');
    if (ev.waitType) lines.push(`- **Wait Type:** ${ev.waitType}`);
    if (ev.resourceType) lines.push(`- **Resource Type:** ${ev.resourceType}`);
    if (ev.lockMode) {
      const desc = getLockModeDescription(ev.lockMode);
      lines.push(`- **Lock Mode:** ${ev.lockMode}${desc ? ` (${desc})` : ''}`);
    }
    if (ev.resourceDescription) lines.push(`- **Resource Description:** ${ev.resourceDescription}`);
    if (ev.waitDurationMs !== null) lines.push(`- **Wait Duration:** ${ev.waitDurationMs}ms`);
    if (ev.extraFields['wait_resource']) lines.push(`- **Wait Resource:** ${ev.extraFields['wait_resource']}`);
    if (ev.extraFields['resolved_wait_object']) lines.push(`- **Resolved Wait Object:** ${ev.extraFields['resolved_wait_object']}`);
    if (ev.extraFields['resolved_object']) lines.push(`- **Resolved Object:** ${ev.extraFields['resolved_object']}`);
  }

  // SQL
  if (ev.statement || ev.sqlText) {
    lines.push('');
    lines.push('## SQL Text');
    lines.push('```sql');
    lines.push(ev.statement || ev.sqlText || '');
    lines.push('```');
  }

  // Deadlock graph
  if (ev.deadlockGraph) {
    lines.push('');
    lines.push('## Deadlock Graph (XML)');
    lines.push('```xml');
    lines.push(ev.deadlockGraph);
    lines.push('```');
  }

  // Blocked process report
  if (ev.blockedProcessReport) {
    lines.push('');
    lines.push('## Blocked Process Report (XML)');
    lines.push('```xml');
    lines.push(ev.blockedProcessReport);
    lines.push('```');
  }

  // Extra fields
  const extras = Object.entries(ev.extraFields);
  if (extras.length > 0) {
    lines.push('');
    lines.push('## Additional Fields');
    for (const [key, val] of extras) {
      lines.push(`- **${key}:** ${val}`);
    }
  }

  // Blocking analysis
  const a = analysis.value;
  if (a) {
    lines.push('');
    lines.push('## Blocking Analysis');
    lines.push(`- **Summary:** ${a.summary}`);
    if (a.diagnosis && a.diagnosis !== 'no_waits') lines.push(`- **Diagnosis:** ${diagnosisLabel(a.diagnosis)}`);

    if (a.blockingChain.length > 0) {
      lines.push('');
      lines.push('### Blocking Chain');
      for (const link of a.blockingChain) {
        lines.push(`- **Session ${link.sessionId}** — Role: ${roleLabel(link.role)}${link.blockedBySession ? `, blocked by Session ${link.blockedBySession}` : ''}`);
        if (link.lockMode) lines.push(`  - Lock Mode: ${link.lockMode}`);
        if (link.waitResource) lines.push(`  - Wait Resource: ${link.waitResource}`);
        if (link.database) lines.push(`  - Database: ${link.database}`);
        if (link.appName) lines.push(`  - App: ${link.appName}`);
        if (link.username) lines.push(`  - User: ${link.username}`);
        if (link.sqlPreview) lines.push(`  - SQL: \`${link.sqlPreview}\``);
      }
    }

    if (a.blockedProcessReports.length > 0) {
      lines.push('');
      lines.push('### Blocked Process Reports');
      for (const bpr of a.blockedProcessReports) {
        lines.push(`- **S${bpr.blockedSpid} blocked by S${bpr.blockingSpid}**`);
        if (bpr.blockedWaitResource) lines.push(`  - Wait Resource: ${bpr.blockedWaitResource}`);
        if (bpr.blockedWaitTimeMs) lines.push(`  - Wait Time: ${bpr.blockedWaitTimeMs}ms`);
        if (bpr.blockedLockMode) lines.push(`  - Victim Lock Mode: ${bpr.blockedLockMode}`);
        if (bpr.blockedIsolationLevel) lines.push(`  - Victim Isolation: ${bpr.blockedIsolationLevel}`);
        if (bpr.blockingIsolationLevel) lines.push(`  - Blocker Isolation: ${bpr.blockingIsolationLevel}`);
        if (bpr.blockingStatus) lines.push(`  - Blocker Status: ${bpr.blockingStatus}`);
        if (bpr.blockingInputBuffer) lines.push(`  - Blocker SQL: \`${bpr.blockingInputBuffer}\``);
        if (bpr.blockedInputBuffer) lines.push(`  - Victim SQL: \`${bpr.blockedInputBuffer}\``);
      }
    }

    if (a.waitStats.length > 0) {
      lines.push('');
      lines.push('### Wait Statistics');
      for (const ws of a.waitStats) {
        lines.push(`- **${ws.waitType}** (${ws.category}): count=${ws.count}, total=${formatDuration(ws.totalDurationUs)}, max=${formatDuration(ws.maxDurationUs)}, avg=${formatDuration(ws.avgDurationUs)}`);
      }
    }

    if (a.recommendations.length > 0) {
      lines.push('');
      lines.push('### Recommendations');
      for (const rec of a.recommendations) {
        lines.push(`- ${rec}`);
      }
    }
  }

  // Related objects
  if (txnObjects.value.length > 0) {
    lines.push('');
    lines.push('## Related Objects (same session/transaction)');
    for (const obj of txnObjects.value) {
      lines.push(`- **${obj.objectName}** — ${obj.eventCount} events, locks: ${obj.lockModes.join(', ') || 'none'}`);
    }
  }

  lines.push('');
  lines.push('---');
  lines.push('Please analyze this event and help me understand what happened, why, and what I can do to fix or prevent it.');

  return lines.join('\n');
};

// Extra fields search
const extraFieldsSearch = ref('');
const filteredExtraFields = computed(() => {
  const event = state.selectedEvent;
  if (!event) return [];
  const entries = Object.entries(event.extraFields);
  const term = extraFieldsSearch.value.toLowerCase();
  if (!term) return entries;
  return entries.filter(([k, v]) =>
    k.toLowerCase().includes(term) || String(v).toLowerCase().includes(term)
  );
});

const highlightText = (text: string, term: string): string => {
  if (!term) return escapeHtml(text);
  const escaped = escapeHtml(text);
  const escapedTerm = escapeHtml(term);
  const regex = new RegExp(`(${escapedTerm.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
  return escaped.replace(regex, '<mark class="bg-yellow-500/30 text-yellow-200 rounded-sm px-0.5">$1</mark>');
};

const escapeHtml = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

// Blocking analysis
const analysis = ref<BlockingAnalysis | null>(null);
const analysisLoading = ref(false);
const analysisError = ref<string | null>(null);
const analysisWindow = ref(60);
const showAnalysis = ref(false);

const isBlockingRelated = (event: XelEvent | null): boolean => {
  if (!event) return false;
  const en = event.eventName;
  return en.includes('lock_') || en.includes('blocked_process') || en.includes('deadlock')
    || en.includes('timeout') || event.waitType !== null
    || event.result === 'Error' || event.result === 'Abort'
    || (event.durationUs !== null && event.cpuTimeUs !== null && event.durationUs > event.cpuTimeUs * 3 && event.durationUs > 1_000_000);
};

const loadAnalysis = async () => {
  if (!state.selectedEvent) return;
  analysisLoading.value = true;
  analysisError.value = null;
  try {
    analysis.value = await xelApi.analyzeBlocking(state.selectedEvent.id, analysisWindow.value * 1000);
    showAnalysis.value = true;
  } catch (e: any) {
    analysisError.value = e?.message || String(e);
  } finally {
    analysisLoading.value = false;
  }
};

const jumpToEvent = async (eventId: number) => {
  const ev = await xelApi.getEvent(eventId);
  if (ev) {
    selectEvent(ev);
  }
};

// Transaction objects correlation (for XACT and unresolved lock events)
const txnObjects = ref<TransactionObject[]>([]);
const txnObjectsLoading = ref(false);

const needsObjectCorrelation = (event: XelEvent | null): boolean => {
  if (!event) return false;
  // Show for XACT resource type, or lock events with no resolved object
  if (event.resourceType === 'XACT') return true;
  if (event.eventName.startsWith('lock_') && !event.objectName && !event.extraFields['resolved_object'] && !event.extraFields['resolved_wait_object']) return true;
  return false;
};

watch(() => state.selectedEvent, async (event) => {
  txnObjects.value = [];
  analysis.value = null;
  showAnalysis.value = false;
  analysisError.value = null;

  if (!event) return;

  if (needsObjectCorrelation(event)) {
    txnObjectsLoading.value = true;
    try {
      txnObjects.value = await xelApi.getTransactionObjects(event.id);
    } catch { /* ignore */ }
    txnObjectsLoading.value = false;
  }

  if (isBlockingRelated(event)) {
    loadAnalysis();
  }
});

const filterBySession = (sessionId: number) => {
  filterByColumn('session_id', String(sessionId));
};

const filterByColumn = (column: string, value: string) => {
  clearFilter();
  setFilter({ textSearch: `${column}:${value}` });
};

const roleColor = (role: string) => {
  switch (role) {
    case 'root_blocker': return 'text-red-400';
    case 'intermediate': return 'text-amber-400';
    case 'victim': return 'text-orange-400';
    default: return 'text-slate-400';
  }
};

const roleIcon = (role: string) => {
  switch (role) {
    case 'root_blocker': return 'fa-hand';
    case 'intermediate': return 'fa-arrows-left-right';
    case 'victim': return 'fa-hourglass-half';
    default: return 'fa-circle';
  }
};

const roleLabel = (role: string) => {
  switch (role) {
    case 'root_blocker': return 'Root Blocker';
    case 'intermediate': return 'Intermediate';
    case 'victim': return 'Victim';
    default: return role;
  }
};

const diagnosisLabel = (d: string) => {
  const labels: Record<string, string> = {
    deadlock: 'Deadlock',
    likely_deadlock: 'Likely Deadlock Victim',
    io_starvation: 'IO Starvation (Disk)',
    lock_blocking: 'Lock Blocking',
    lock_contention: 'Lock Contention',
    latch_contention: 'Latch Contention (Hot Pages)',
    network_bottleneck: 'Network Bottleneck',
    memory_pressure: 'Memory Pressure',
    cpu_pressure: 'CPU Pressure',
    unknown_wait: 'Unknown Wait',
    no_waits: 'No Waits Detected',
    mixed: 'Mixed Waits',
  };
  return labels[d] || d;
};

const categoryDotColor = (cat: string) => {
  const colors: Record<string, string> = {
    io: 'bg-blue-400',
    lock: 'bg-red-400',
    latch: 'bg-amber-400',
    network: 'bg-purple-400',
    cpu: 'bg-cyan-400',
    memory: 'bg-pink-400',
    idle: 'bg-slate-500',
    other: 'bg-slate-400',
  };
  return colors[cat] || 'bg-slate-400';
};

const categoryBarColor = (cat: string) => {
  const colors: Record<string, string> = {
    io: 'bg-blue-500',
    lock: 'bg-red-500',
    latch: 'bg-amber-500',
    network: 'bg-purple-500',
    cpu: 'bg-cyan-500',
    memory: 'bg-pink-500',
    idle: 'bg-slate-600',
    other: 'bg-slate-500',
  };
  return colors[cat] || 'bg-slate-500';
};

const waitCategoryBreakdown = computed(() => {
  if (!analysis.value?.waitStats.length) return [];
  const cats: Record<string, number> = {};
  let total = 0;
  for (const ws of analysis.value.waitStats) {
    cats[ws.category] = (cats[ws.category] || 0) + ws.totalDurationUs;
    total += ws.totalDurationUs;
  }
  if (total === 0) return [];
  return Object.entries(cats)
    .map(([category, dur]) => ({ category, pct: Math.round((dur / total) * 100) }))
    .filter(c => c.pct > 0)
    .sort((a, b) => b.pct - a.pct);
});

</script>

<template>
  <div class="flex flex-col h-full bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <template v-if="state.selectedEvent">
      <!-- Header -->
      <div class="shrink-0 px-4 py-3 bg-slate-700 border-b border-slate-600">
        <div class="flex items-center gap-2">
          <i
            :class="['fa-solid', getEventIcon(state.selectedEvent.eventName)]"
            :style="{ color: getEventSeverityColor(getEventSeverity(state.selectedEvent)) }"
          ></i>
          <h3 class="text-sm font-semibold text-slate-200 truncate flex-1">
            {{ state.selectedEvent.eventName }}
          </h3>
          <button
            @click="copyText(buildLlmPrompt(), 'llm')"
            class="shrink-0 flex items-center gap-1.5 px-2 py-1 rounded-lg text-xs font-medium transition-colors"
            :class="copied === 'llm' ? 'bg-green-600/30 text-green-300' : 'bg-indigo-600/30 text-indigo-300 hover:bg-indigo-600/50 hover:text-indigo-200'"
            title="Copy all event details formatted for LLM analysis"
          >
            <i :class="copied === 'llm' ? 'fa-solid fa-check' : 'fa-solid fa-robot'" class="text-[10px]"></i>
            {{ copied === 'llm' ? 'Copied!' : 'Copy for AI' }}
          </button>
        </div>
        <p class="text-xs text-slate-400 mt-1">
          {{ formatTimestampFull(state.selectedEvent.timestamp) }}
        </p>
      </div>

      <!-- Metrics -->
      <div class="flex-1 overflow-auto px-4 py-3 space-y-3">
        <!-- Blocking Analysis (auto-loaded) -->
        <div v-if="isBlockingRelated(state.selectedEvent)">
          <div v-if="analysisLoading" class="flex items-center gap-2 text-xs text-slate-400 py-1">
            <i class="fa-solid fa-spinner fa-spin text-[10px]"></i>
            Analyzing blocking...
          </div>

          <!-- Analysis Results -->
          <div v-if="showAnalysis && analysis" class="space-y-3">
            <div class="flex items-center justify-between">
              <h4 class="text-xs font-semibold text-indigo-400 uppercase tracking-wider">
                <i class="fa-solid fa-link mr-1"></i>Blocking Analysis
              </h4>
              <div class="flex items-center gap-1">
                <select
                  v-model.number="analysisWindow"
                  @change="loadAnalysis"
                  class="bg-slate-700 text-slate-300 border border-slate-600 rounded px-1 py-0.5 outline-none text-xs"
                >
                  <option :value="15">15s</option>
                  <option :value="30">30s</option>
                  <option :value="60">1min</option>
                  <option :value="120">2min</option>
                  <option :value="300">5min</option>
                </select>
                <button
                  @click="showAnalysis = false; analysis = null"
                  class="text-xs text-slate-500 hover:text-slate-300 px-1"
                >
                  <i class="fa-solid fa-xmark"></i>
                </button>
              </div>
            </div>

            <!-- Summary -->
            <div class="text-xs text-slate-300 bg-slate-900/50 rounded-lg px-3 py-2 leading-relaxed">
              {{ analysis.summary }}
            </div>

            <!-- Deadlock Graphs -->
            <div v-if="analysis.deadlocks.length > 0">
              <h5 class="text-xs font-semibold text-red-400 uppercase tracking-wider mb-1.5">
                <i class="fa-solid fa-skull-crossbones mr-1 text-[10px]"></i>
                Deadlock{{ analysis.deadlocks.length > 1 ? 's' : '' }} ({{ analysis.deadlocks.length }})
              </h5>
              <div class="space-y-2">
                <div
                  v-for="dl in analysis.deadlocks"
                  :key="dl.eventId"
                  class="bg-red-950/20 border border-red-800/30 rounded-lg px-3 py-2 space-y-2"
                >
                  <!-- Processes -->
                  <div class="space-y-1.5">
                    <div
                      v-for="proc in dl.processes"
                      :key="proc.id"
                      class="text-xs"
                    >
                      <div class="flex items-center gap-1.5 mb-0.5">
                        <i :class="proc.isVictim ? 'fa-solid fa-skull text-red-400' : 'fa-solid fa-hand text-amber-400'" class="text-[10px]"></i>
                        <span :class="proc.isVictim ? 'text-red-400' : 'text-amber-400'" class="font-semibold">
                          {{ proc.isVictim ? 'Victim' : 'Holder' }}
                        </span>
                        <span class="text-slate-400">
                          Session {{ proc.spid ?? '?' }}
                        </span>
                        <span v-if="proc.appName" class="text-slate-500 truncate text-[10px]">({{ proc.appName }})</span>
                      </div>
                      <div class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-slate-400 ml-4">
                        <template v-if="proc.waitResource">
                          <span>Wait resource</span>
                          <span class="text-yellow-300 font-mono text-[10px]">{{ proc.waitResource }}</span>
                        </template>
                        <template v-if="proc.lockMode">
                          <span>Lock mode</span>
                          <span class="text-yellow-300" :title="getLockModeDescription(proc.lockMode) ?? ''">{{ proc.lockMode }}<span v-if="getLockModeDescription(proc.lockMode)" class="text-yellow-300/50 text-[9px] ml-1">{{ getLockModeDescription(proc.lockMode) }}</span></span>
                        </template>
                        <template v-if="proc.isolationLevel">
                          <span>Isolation</span>
                          <span class="text-slate-300">{{ proc.isolationLevel }}</span>
                        </template>
                        <template v-if="proc.transactionName">
                          <span>Transaction</span>
                          <span class="text-slate-300">{{ proc.transactionName }}</span>
                        </template>
                        <template v-if="proc.waitTimeMs">
                          <span>Wait time</span>
                          <span class="text-slate-300">{{ proc.waitTimeMs }}ms</span>
                        </template>
                      </div>
                      <div v-if="proc.inputBuffer" class="mt-1 ml-4">
                        <pre class="text-[10px] font-mono px-2 py-1 rounded overflow-auto max-h-16 whitespace-pre-wrap break-all"
                          :class="proc.isVictim ? 'text-red-300/70 bg-red-950/40' : 'text-amber-300/70 bg-amber-950/30'"
                        >{{ proc.inputBuffer }}</pre>
                      </div>
                      <div v-if="proc.spid" class="mt-1 ml-4">
                        <button
                          @click="filterBySession(proc.spid!)"
                          class="text-[10px] text-indigo-400 hover:text-indigo-300"
                        >
                          <i class="fa-solid fa-filter mr-0.5"></i>Filter Session {{ proc.spid }}
                        </button>
                      </div>
                    </div>
                  </div>

                  <!-- Resources -->
                  <div v-if="dl.resources.length > 0">
                    <div class="text-[10px] text-slate-500 font-semibold uppercase mb-0.5">Contended Resources</div>
                    <div class="space-y-0.5">
                      <div
                        v-for="(res, ri) in dl.resources"
                        :key="ri"
                        class="text-[10px] text-slate-400 bg-slate-800/50 rounded px-2 py-1"
                      >
                        <span class="text-yellow-300 font-medium">{{ res.resourceType }}</span>
                        <span v-if="res.objectName" class="text-slate-300 ml-1">{{ res.objectName }}</span>
                        <span v-if="res.indexName" class="text-slate-500 ml-1">({{ res.indexName }})</span>
                        <span v-if="res.mode" class="text-slate-500 ml-1">mode: {{ res.mode }}</span>
                        <div class="flex gap-2 mt-0.5">
                          <span v-for="h in res.holders" :key="'h-' + h.processId" class="text-green-400">
                            holds: {{ h.mode }}
                          </span>
                          <span v-for="w in res.waiters" :key="'w-' + w.processId" class="text-red-400">
                            waits: {{ w.mode }}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  <button
                    @click="jumpToEvent(dl.eventId)"
                    class="text-[10px] text-indigo-400 hover:text-indigo-300"
                  >
                    View deadlock event #{{ dl.eventId }}
                  </button>
                </div>
              </div>
            </div>

            <!-- Deadlock Lock Events (confirmed via deadlock_id) -->
            <div v-if="analysis.deadlockId && analysis.deadlockLockEvents.length > 0">
              <h5 class="text-xs font-semibold text-red-400 uppercase tracking-wider mb-1.5">
                <i class="fa-solid fa-skull-crossbones mr-1 text-[10px]"></i>
                Deadlock Lock Events
                <span class="text-red-300/70 font-normal ml-1">(deadlock_id: {{ analysis.deadlockId }})</span>
              </h5>
              <div class="space-y-1.5">
                <div
                  v-for="ev in analysis.deadlockLockEvents"
                  :key="ev.id"
                  class="bg-red-950/20 border border-red-800/30 rounded-lg px-3 py-2 text-xs"
                >
                  <div class="flex items-center justify-between mb-1">
                    <div class="flex items-center gap-1.5">
                      <i
                        :class="['fa-solid', getEventIcon(ev.eventName)]"
                        :style="{ color: getEventSeverityColor(getEventSeverity(ev)) }"
                        class="text-[10px]"
                      ></i>
                      <span class="text-red-300 font-semibold">{{ ev.eventName }}</span>
                      <span class="text-slate-400">Session {{ ev.sessionId ?? '?' }}</span>
                    </div>
                    <button
                      @click="jumpToEvent(ev.id)"
                      class="text-[10px] text-indigo-400 hover:text-indigo-300"
                    >
                      #{{ ev.id }}
                    </button>
                  </div>
                  <div class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-slate-400">
                    <template v-if="ev.resourceType">
                      <span>Resource Type</span>
                      <span class="text-yellow-300 font-medium">{{ ev.resourceType }}</span>
                    </template>
                    <template v-if="ev.lockMode">
                      <span>Lock Mode</span>
                      <span class="text-yellow-300" :title="getLockModeDescription(ev.lockMode) ?? ''">{{ ev.lockMode }}<span v-if="getLockModeDescription(ev.lockMode)" class="text-yellow-300/50 text-[9px] ml-1">{{ getLockModeDescription(ev.lockMode) }}</span></span>
                    </template>
                    <template v-if="ev.objectName">
                      <span>Object</span>
                      <span class="text-slate-300">{{ ev.objectName }}</span>
                    </template>
                    <template v-if="ev.resourceDescription">
                      <span>Resource</span>
                      <span class="text-slate-300 font-mono text-[10px]">{{ ev.resourceDescription }}</span>
                    </template>
                    <template v-if="ev.durationUs !== null">
                      <span>Duration</span>
                      <span class="text-slate-300">{{ formatDuration(ev.durationUs) }}</span>
                    </template>
                    <template v-if="ev.databaseName">
                      <span>Database</span>
                      <span class="text-slate-300">{{ ev.databaseName }}</span>
                    </template>
                    <template v-if="ev.username">
                      <span>User</span>
                      <span class="text-slate-300">{{ ev.username }}</span>
                    </template>
                    <template v-if="ev.extraFields['owner_type']">
                      <span>Owner Type</span>
                      <span class="text-slate-300">{{ ev.extraFields['owner_type'] }}</span>
                    </template>
                  </div>
                  <!-- Show all extra fields for this deadlock event -->
                  <details class="mt-1.5">
                    <summary class="text-[10px] text-slate-500 hover:text-slate-300 cursor-pointer">
                      All fields ({{ Object.keys(ev.extraFields).length }})
                    </summary>
                    <div class="mt-1 space-y-0 max-h-32 overflow-auto">
                      <div
                        v-for="[key, val] in Object.entries(ev.extraFields)"
                        :key="key"
                        class="grid grid-cols-[1fr_2fr] gap-1 text-[10px] border-b border-slate-700/50 py-0.5"
                      >
                        <span class="text-slate-500 font-mono">{{ key }}</span>
                        <span class="text-slate-300 font-mono break-all">{{ val }}</span>
                      </div>
                    </div>
                  </details>
                  <!-- SQL Text if available -->
                  <div v-if="ev.sqlText || ev.statement" class="mt-1.5">
                    <pre class="text-[10px] text-red-300/70 font-mono bg-red-950/40 px-2 py-1 rounded overflow-auto max-h-16 whitespace-pre-wrap break-all">{{ ev.statement || ev.sqlText }}</pre>
                  </div>
                  <div class="mt-1 flex flex-wrap gap-2">
                    <button
                      v-if="ev.extraFields['attach_activity_id']"
                      @click="filterByColumn('attach_activity_id', String(ev.extraFields['attach_activity_id']).split(':')[0])"
                      class="text-[10px] text-indigo-400 hover:text-indigo-300"
                      :title="String(ev.extraFields['attach_activity_id']).split(':')[0]"
                    >
                      <i class="fa-solid fa-link mr-0.5"></i>Activity
                    </button>
                    <button
                      v-if="ev.extraFields['transaction_id']"
                      @click="filterByColumn('transaction_id', String(ev.extraFields['transaction_id']))"
                      class="text-[10px] text-slate-500 hover:text-slate-300"
                    >
                      <i class="fa-solid fa-filter mr-0.5"></i>Txn
                    </button>
                    <button
                      v-if="ev.extraFields['deadlock_id']"
                      @click="filterByColumn('deadlock_id', String(ev.extraFields['deadlock_id']))"
                      class="text-[10px] text-red-400 hover:text-red-300"
                    >
                      <i class="fa-solid fa-skull-crossbones mr-0.5"></i>Deadlock
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Blocking Chain -->
            <div v-if="analysis.blockingChain.length > 0">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Blocking Chain</h5>
              <div class="space-y-1.5">
                <div
                  v-for="link in analysis.blockingChain"
                  :key="link.sessionId"
                  class="bg-slate-700/50 rounded-lg px-3 py-2 text-xs border border-slate-600/50"
                >
                  <div class="flex items-center justify-between mb-1">
                    <div class="flex items-center gap-1.5">
                      <i :class="['fa-solid', roleIcon(link.role), roleColor(link.role)]" class="text-[10px]"></i>
                      <span :class="roleColor(link.role)" class="font-semibold">
                        {{ roleLabel(link.role) }}
                      </span>
                      <span class="text-slate-400">Session {{ link.sessionId }}</span>
                    </div>
                    <button
                      @click="filterBySession(link.sessionId)"
                      class="text-indigo-400 hover:text-indigo-300 text-[10px]"
                      title="Filter by this session"
                    >
                      <i class="fa-solid fa-filter"></i>
                    </button>
                  </div>

                  <div class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-slate-400">
                    <template v-if="link.blockedBySession">
                      <span>Blocked by</span>
                      <span class="text-red-400">Session {{ link.blockedBySession }}</span>
                    </template>
                    <template v-if="link.waitResource">
                      <span>Resource</span>
                      <span class="text-yellow-300 font-mono text-[10px]">{{ link.waitResource }}</span>
                    </template>
                    <template v-if="link.lockMode">
                      <span>Lock mode</span>
                      <span class="text-yellow-300" :title="getLockModeDescription(link.lockMode) ?? ''">{{ link.lockMode }}<span v-if="getLockModeDescription(link.lockMode)" class="text-yellow-300/50 text-[9px] ml-1">{{ getLockModeDescription(link.lockMode) }}</span></span>
                    </template>
                    <template v-if="link.appName">
                      <span>App</span>
                      <span class="text-slate-300 truncate">{{ link.appName }}</span>
                    </template>
                    <template v-if="link.username">
                      <span>User</span>
                      <span class="text-slate-300">{{ link.username }}</span>
                    </template>
                  </div>

                  <!-- SQL Preview -->
                  <div v-if="link.sqlPreview" class="mt-1.5">
                    <pre class="text-[10px] text-slate-400 font-mono bg-slate-900/50 px-2 py-1 rounded overflow-auto max-h-20 whitespace-pre-wrap break-all">{{ link.sqlPreview }}</pre>
                  </div>

                  <!-- Jump to events -->
                  <div v-if="link.eventIds.length > 0" class="mt-1.5 flex flex-wrap items-center gap-1">
                    <span class="text-[10px] text-slate-500">Session events:</span>
                    <button
                      v-for="eid in link.eventIds.slice(0, 5)"
                      :key="eid"
                      @click="jumpToEvent(eid)"
                      class="text-[10px] px-1.5 py-0.5 rounded bg-slate-600/50 text-indigo-300 hover:bg-slate-600 hover:text-indigo-200 transition-colors"
                    >
                      #{{ eid }}
                    </button>
                    <span v-if="link.eventIds.length > 5" class="text-[10px] text-slate-500">
                      +{{ link.eventIds.length - 5 }} more
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Blocked Process Reports -->
            <div v-if="analysis.blockedProcessReports.length > 0">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
                Blocked Process Reports ({{ analysis.blockedProcessReports.length }})
              </h5>
              <div class="space-y-1.5">
                <div
                  v-for="bpr in analysis.blockedProcessReports"
                  :key="bpr.eventId"
                  class="bg-slate-700/50 rounded-lg px-3 py-2 text-xs border border-orange-800/30"
                >
                  <div class="flex items-center justify-between mb-1">
                    <span class="text-orange-400 font-medium">
                      <i class="fa-solid fa-ban mr-1 text-[10px]"></i>
                      S{{ bpr.blockedSpid }} blocked by S{{ bpr.blockingSpid }}
                    </span>
                    <button
                      @click="jumpToEvent(bpr.eventId)"
                      class="text-[10px] text-indigo-400 hover:text-indigo-300"
                    >
                      #{{ bpr.eventId }}
                    </button>
                  </div>
                  <div class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-slate-400">
                    <template v-if="bpr.blockedWaitResource">
                      <span>Wait resource</span>
                      <span class="text-yellow-300 font-mono text-[10px]">
                        {{ state.selectedEvent?.extraFields['resolved_wait_resource'] || bpr.blockedWaitResource }}
                      </span>
                    </template>
                    <template v-if="bpr.blockedWaitTimeMs">
                      <span>Wait time</span>
                      <span class="text-slate-300">{{ bpr.blockedWaitTimeMs.toLocaleString() }}ms</span>
                    </template>
                    <template v-if="bpr.blockedLockMode">
                      <span>Lock mode</span>
                      <span class="text-yellow-300">{{ bpr.blockedLockMode }}</span>
                    </template>
                    <template v-if="bpr.blockedIsolationLevel">
                      <span>Victim isolation</span>
                      <span class="text-slate-300">{{ bpr.blockedIsolationLevel }}</span>
                    </template>
                    <template v-if="bpr.blockedTranCount">
                      <span>Victim tran count</span>
                      <span class="text-slate-300">{{ bpr.blockedTranCount }}</span>
                    </template>
                    <template v-if="bpr.blockingStatus">
                      <span>Blocker status</span>
                      <span class="text-slate-300">{{ bpr.blockingStatus }}</span>
                    </template>
                    <template v-if="bpr.blockingLastBatchStarted">
                      <span>Blocker started</span>
                      <span class="text-slate-300">{{ bpr.blockingLastBatchStarted }}</span>
                    </template>
                    <template v-if="bpr.blockingIsolationLevel">
                      <span>Blocker isolation</span>
                      <span class="text-slate-300">{{ bpr.blockingIsolationLevel }}</span>
                    </template>
                    <template v-if="bpr.blockingTranCount">
                      <span>Blocker tran count</span>
                      <span class="text-slate-300">{{ bpr.blockingTranCount }}</span>
                    </template>
                  </div>
                  <!-- Execution Stack -->
                  <div v-if="bpr.blockedExecutionStack.some(f => f.queryHash)" class="mt-1.5">
                    <span class="text-slate-500 text-[10px]">Victim execution stack:</span>
                    <template v-for="(frame, fi) in bpr.blockedExecutionStack" :key="fi">
                      <div v-if="frame.queryHash" class="text-[10px] font-mono text-slate-400 bg-slate-900/40 px-2 py-0.5 rounded mt-0.5">
                        <span class="text-indigo-300">{{ frame.queryHash }}</span>
                        <span v-if="frame.queryPlanHash" class="text-slate-500 ml-1">plan: {{ frame.queryPlanHash }}</span>
                        <span v-if="frame.line" class="text-slate-500 ml-1">line {{ frame.line }}</span>
                      </div>
                    </template>
                  </div>
                  <div v-if="bpr.blockingExecutionStack.some(f => f.queryHash)" class="mt-1">
                    <span class="text-slate-500 text-[10px]">Blocker execution stack:</span>
                    <template v-for="(frame, fi) in bpr.blockingExecutionStack" :key="fi">
                      <div v-if="frame.queryHash" class="text-[10px] font-mono text-slate-400 bg-slate-900/40 px-2 py-0.5 rounded mt-0.5">
                        <span class="text-red-300">{{ frame.queryHash }}</span>
                        <span v-if="frame.queryPlanHash" class="text-slate-500 ml-1">plan: {{ frame.queryPlanHash }}</span>
                        <span v-if="frame.line" class="text-slate-500 ml-1">line {{ frame.line }}</span>
                      </div>
                    </template>
                  </div>
                  <!-- Blocker's SQL -->
                  <div v-if="bpr.blockingInputBuffer" class="mt-1.5">
                    <span class="text-slate-500 text-[10px]">Blocker SQL:</span>
                    <pre class="text-[10px] text-red-300/70 font-mono bg-red-950/30 px-2 py-1 rounded overflow-auto max-h-16 whitespace-pre-wrap break-all">{{ bpr.blockingInputBuffer }}</pre>
                  </div>
                  <!-- Victim's SQL -->
                  <div v-if="bpr.blockedInputBuffer" class="mt-1">
                    <span class="text-slate-500 text-[10px]">Victim SQL:</span>
                    <pre class="text-[10px] text-orange-300/70 font-mono bg-orange-950/20 px-2 py-1 rounded overflow-auto max-h-16 whitespace-pre-wrap break-all">{{ bpr.blockedInputBuffer }}</pre>
                  </div>
                </div>
              </div>
            </div>

            <!-- Blocker Events -->
            <div v-if="analysis.blockerEvents.length > 0">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
                Blocker Session Events ({{ analysis.blockerEvents.length }})
              </h5>
              <div class="space-y-0.5 max-h-40 overflow-auto">
                <button
                  v-for="ev in analysis.blockerEvents.slice(0, 20)"
                  :key="ev.id"
                  @click="jumpToEvent(ev.id)"
                  class="w-full text-left px-2 py-1 rounded text-[10px] hover:bg-slate-700/50 transition-colors flex items-center gap-2"
                >
                  <i
                    :class="['fa-solid', getEventIcon(ev.eventName)]"
                    :style="{ color: getEventSeverityColor(getEventSeverity(ev)) }"
                    class="text-[9px]"
                  ></i>
                  <span class="text-slate-400 shrink-0">S{{ ev.sessionId }}</span>
                  <span class="text-slate-300 truncate">{{ ev.eventName }}</span>
                  <span v-if="ev.durationUs" class="text-slate-500 shrink-0 ml-auto">{{ formatDuration(ev.durationUs) }}</span>
                </button>
              </div>
            </div>

            <!-- Lock Escalations -->
            <div v-if="analysis.lockEscalations.length > 0">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
                <i class="fa-solid fa-arrow-up text-amber-400 mr-1 text-[10px]"></i>
                Lock Escalations ({{ analysis.lockEscalations.length }})
              </h5>
              <div class="space-y-0.5">
                <button
                  v-for="ev in analysis.lockEscalations"
                  :key="ev.id"
                  @click="jumpToEvent(ev.id)"
                  class="w-full text-left px-2 py-1 rounded text-[10px] hover:bg-slate-700/50 transition-colors text-amber-300"
                >
                  S{{ ev.sessionId }} - {{ ev.objectName || 'unknown' }} ({{ ev.resourceType }})
                </button>
              </div>
            </div>

            <!-- Diagnosis -->
            <div v-if="analysis.diagnosis && analysis.diagnosis !== 'no_waits'">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
                <i :class="[
                  'fa-solid mr-1 text-[10px]',
                  analysis.diagnosis === 'deadlock' || analysis.diagnosis === 'likely_deadlock' ? 'fa-skull-crossbones text-red-400' :
                  analysis.diagnosis === 'io_starvation' ? 'fa-hard-drive text-blue-400' :
                  analysis.diagnosis.startsWith('lock') ? 'fa-lock text-red-400' :
                  analysis.diagnosis === 'latch_contention' ? 'fa-bolt text-amber-400' :
                  analysis.diagnosis === 'network_bottleneck' ? 'fa-network-wired text-purple-400' :
                  analysis.diagnosis === 'memory_pressure' ? 'fa-memory text-pink-400' :
                  analysis.diagnosis === 'cpu_pressure' ? 'fa-microchip text-cyan-400' :
                  'fa-question-circle text-slate-400'
                ]"></i>
                Diagnosis: {{ diagnosisLabel(analysis.diagnosis) }}
              </h5>
            </div>

            <!-- Wait Stats (aggregated) -->
            <div v-if="analysis.waitStats.length > 0">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
                Wait Statistics ({{ analysis.waitEvents.length }} events)
              </h5>
              <div class="space-y-1">
                <div
                  v-for="ws in analysis.waitStats"
                  :key="ws.waitType"
                  class="flex items-center gap-2 text-xs px-2 py-1 rounded bg-slate-700/30"
                >
                  <span
                    class="w-1.5 h-1.5 rounded-full shrink-0"
                    :class="categoryDotColor(ws.category)"
                  ></span>
                  <span class="text-slate-300 font-mono text-[10px] min-w-0 truncate flex-1">{{ ws.waitType }}</span>
                  <span class="text-slate-500 shrink-0">x{{ ws.count }}</span>
                  <span class="text-slate-400 shrink-0 font-medium tabular-nums">{{ formatDuration(ws.totalDurationUs) }}</span>
                </div>
              </div>
              <!-- Category breakdown bar -->
              <div v-if="analysis.waitStats.length > 1" class="mt-2">
                <div class="flex h-2 rounded-full overflow-hidden bg-slate-700">
                  <div
                    v-for="cat in waitCategoryBreakdown"
                    :key="cat.category"
                    :class="categoryBarColor(cat.category)"
                    :style="{ width: cat.pct + '%' }"
                    :title="`${cat.category}: ${cat.pct}%`"
                  ></div>
                </div>
                <div class="flex flex-wrap gap-x-3 gap-y-0.5 mt-1">
                  <span
                    v-for="cat in waitCategoryBreakdown"
                    :key="cat.category"
                    class="text-[10px] text-slate-500 flex items-center gap-1"
                  >
                    <span class="w-1.5 h-1.5 rounded-full" :class="categoryDotColor(cat.category)"></span>
                    {{ cat.category }} {{ cat.pct }}%
                  </span>
                </div>
              </div>
            </div>

            <!-- Recommendations -->
            <div v-if="analysis.recommendations.length > 0">
              <h5 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
                <i class="fa-solid fa-lightbulb text-amber-400 mr-1 text-[10px]"></i>
                Recommendations
              </h5>
              <ul class="space-y-1.5">
                <li
                  v-for="(rec, i) in analysis.recommendations"
                  :key="i"
                  class="text-xs text-slate-300 bg-slate-900/40 rounded px-2.5 py-1.5 leading-relaxed"
                >
                  {{ rec }}
                </li>
              </ul>
            </div>
          </div>

          <!-- Analysis Error -->
          <div v-if="analysisError" class="text-xs text-red-400 mt-1">
            {{ analysisError }}
          </div>
        </div>

        <!-- Identity -->
        <section>
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Identity</h4>
          <div class="grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
            <span class="text-slate-500">Event ID</span>
            <span class="text-slate-300">{{ state.selectedEvent.id }}</span>
            <span class="text-slate-500">Session</span>
            <span class="text-slate-300">{{ state.selectedEvent.sessionId ?? '-' }}</span>
            <span class="text-slate-500">User</span>
            <span class="text-slate-300 truncate">{{ state.selectedEvent.username ?? '-' }}</span>
            <span class="text-slate-500">App</span>
            <span class="text-slate-300 truncate">{{ state.selectedEvent.clientAppName ?? '-' }}</span>
            <span class="text-slate-500">Database</span>
            <span class="text-slate-300 truncate">{{ state.selectedEvent.databaseName ?? '-' }}</span>
          </div>
        </section>

        <!-- Performance -->
        <section v-if="state.selectedEvent.durationUs !== null || state.selectedEvent.logicalReads !== null">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Performance</h4>
          <div class="grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
            <span class="text-slate-500">Duration</span>
            <span class="text-slate-300 font-medium">{{ formatDuration(state.selectedEvent.durationUs) }}</span>
            <template v-if="state.selectedEvent.cpuTimeUs !== null">
              <span class="text-slate-500">CPU Time</span>
              <span class="text-slate-300">{{ formatDuration(state.selectedEvent.cpuTimeUs) }}</span>
            </template>
            <!-- Duration vs CPU gap indicator -->
            <template v-if="state.selectedEvent.durationUs !== null && state.selectedEvent.cpuTimeUs !== null && state.selectedEvent.durationUs > state.selectedEvent.cpuTimeUs * 2">
              <span class="text-slate-500">Wait ratio</span>
              <span class="text-orange-300 font-medium">
                {{ Math.round((1 - state.selectedEvent.cpuTimeUs / state.selectedEvent.durationUs) * 100) }}% waiting
              </span>
            </template>
            <template v-if="state.selectedEvent.logicalReads !== null">
              <span class="text-slate-500">Logical Reads</span>
              <span class="text-slate-300">{{ formatNumber(state.selectedEvent.logicalReads) }}</span>
            </template>
            <template v-if="state.selectedEvent.physicalReads !== null">
              <span class="text-slate-500">Physical Reads</span>
              <span class="text-slate-300">{{ formatNumber(state.selectedEvent.physicalReads) }}</span>
            </template>
            <template v-if="state.selectedEvent.writes !== null">
              <span class="text-slate-500">Writes</span>
              <span class="text-slate-300">{{ formatNumber(state.selectedEvent.writes) }}</span>
            </template>
            <template v-if="state.selectedEvent.result">
              <span class="text-slate-500">Result</span>
              <span :class="state.selectedEvent.result === 'Error' ? 'text-red-400' : 'text-slate-300'">
                {{ state.selectedEvent.result }}
              </span>
            </template>
          </div>
        </section>

        <!-- Lock Info -->
        <section v-if="state.selectedEvent.resourceType || state.selectedEvent.lockMode || state.selectedEvent.waitType || state.selectedEvent.extraFields['wait_resource']">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Lock / Wait</h4>
          <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
            <template v-if="state.selectedEvent.waitType">
              <span class="text-slate-500">Wait Type</span>
              <span class="text-orange-300 font-medium">{{ state.selectedEvent.waitType }}</span>
            </template>
            <template v-if="state.selectedEvent.resourceType">
              <span class="text-slate-500">Resource Type</span>
              <span class="text-yellow-300 font-medium">{{ state.selectedEvent.resourceType }}</span>
            </template>
            <template v-if="state.selectedEvent.lockMode">
              <span class="text-slate-500">Lock Mode</span>
              <span class="text-yellow-300 font-medium" :title="getLockModeDescription(state.selectedEvent.lockMode) ?? ''">{{ state.selectedEvent.lockMode }}<span v-if="getLockModeDescription(state.selectedEvent.lockMode)" class="text-yellow-300/50 text-[10px] font-normal ml-1.5">{{ getLockModeDescription(state.selectedEvent.lockMode) }}</span></span>
            </template>
            <template v-if="state.selectedEvent.extraFields['wait_resource']">
              <span class="text-slate-500">Wait Resource</span>
              <span class="text-slate-300 font-mono text-[10px]">{{ state.selectedEvent.extraFields['wait_resource'] }}</span>
            </template>
            <template v-if="state.selectedEvent.extraFields['resolved_wait_object']">
              <span class="text-slate-500">Resolved Object</span>
              <span class="text-emerald-400 font-medium">{{ state.selectedEvent.extraFields['resolved_wait_object'] }}</span>
            </template>
            <template v-if="state.selectedEvent.extraFields['resolved_object']">
              <span class="text-slate-500">Resolved Object</span>
              <span class="text-emerald-400 font-medium">{{ state.selectedEvent.extraFields['resolved_object'] }}</span>
            </template>
            <template v-if="state.selectedEvent.resourceDescription">
              <span class="text-slate-500">Resource Desc</span>
              <span class="text-slate-400 truncate font-mono text-[10px]">{{ state.selectedEvent.resourceDescription }}</span>
            </template>
            <template v-if="state.selectedEvent.waitDurationMs !== null">
              <span class="text-slate-500">Wait Duration</span>
              <span class="text-slate-300">{{ state.selectedEvent.waitDurationMs?.toLocaleString() }}ms</span>
            </template>
          </div>
        </section>

        <!-- Related Objects (from same session/transaction) -->
        <section v-if="txnObjectsLoading || txnObjects.length > 0">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
            <i class="fa-solid fa-link text-emerald-400 mr-1"></i>Related Objects
            <span class="text-slate-600 font-normal ml-1">(same session/transaction)</span>
          </h4>
          <div v-if="txnObjectsLoading" class="text-xs text-slate-500">
            <i class="fa-solid fa-spinner fa-spin mr-1"></i>Loading...
          </div>
          <div v-else class="space-y-1">
            <div
              v-for="obj in txnObjects"
              :key="obj.objectName"
              class="flex items-center justify-between text-xs bg-slate-700/50 px-2 py-1.5 rounded hover:bg-slate-700 transition-colors cursor-pointer"
              @click="jumpToEvent(obj.sampleEventId)"
            >
              <div class="min-w-0 flex-1">
                <span class="text-emerald-400 font-medium">{{ obj.objectName }}</span>
                <span v-if="obj.lockModes.length" class="text-yellow-300/70 ml-2" :title="obj.lockModes.map(m => m + ': ' + (getLockModeDescription(m) || '?')).join('\n')">{{ obj.lockModes.join(', ') }}</span>
              </div>
              <span class="text-slate-500 ml-2 shrink-0">{{ obj.eventCount }} event{{ obj.eventCount !== 1 ? 's' : '' }}</span>
            </div>
          </div>
        </section>

        <!-- Object -->
        <section v-if="state.selectedEvent.objectName">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Object</h4>
          <p class="text-xs text-slate-300 font-mono bg-slate-700/50 px-2 py-1 rounded">
            {{ state.selectedEvent.objectName }}
          </p>
        </section>

        <!-- SQL Text -->
        <section v-if="state.selectedEvent.sqlText || state.selectedEvent.statement">
          <div class="flex items-center justify-between mb-1.5">
            <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider">SQL Text</h4>
            <button
              @click="copyText(state.selectedEvent!.statement || state.selectedEvent!.sqlText || '', 'sql')"
              class="text-xs px-1.5 py-0.5 rounded text-slate-500 hover:text-slate-200 hover:bg-slate-700 transition-colors"
            >
              <i :class="copied === 'sql' ? 'fa-solid fa-check text-green-400' : 'fa-regular fa-copy'"></i>
            </button>
          </div>
          <pre class="text-xs text-slate-300 font-mono bg-slate-900/50 px-3 py-2 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap break-all">{{ state.selectedEvent.statement || state.selectedEvent.sqlText }}</pre>
        </section>

        <!-- Deadlock Graph -->
        <section v-if="state.selectedEvent.deadlockGraph">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
            <i class="fa-solid fa-skull-crossbones text-red-400 mr-1"></i>Deadlock Graph
          </h4>
          <pre class="text-xs text-slate-400 font-mono bg-slate-900/50 px-3 py-2 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap break-all">{{ state.selectedEvent.deadlockGraph }}</pre>
        </section>

        <!-- Blocked Process Report -->
        <section v-if="state.selectedEvent.blockedProcessReport">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
            <i class="fa-solid fa-ban text-orange-400 mr-1"></i>Blocked Process Report (raw)
          </h4>
          <pre class="text-xs text-slate-400 font-mono bg-slate-900/50 px-3 py-2 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap break-all">{{ state.selectedEvent.blockedProcessReport }}</pre>
        </section>

        <!-- Extra Fields -->
        <section v-if="Object.keys(state.selectedEvent.extraFields).length > 0">
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">
            Additional Fields
            <span class="text-slate-600 font-normal ml-1">({{ Object.keys(state.selectedEvent.extraFields).length }})</span>
          </h4>
          <!-- Search -->
          <div class="relative mb-2">
            <i class="fa-solid fa-magnifying-glass absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400 text-xs pointer-events-none"></i>
            <input
              v-model="extraFieldsSearch"
              type="text"
              placeholder="Search properties..."
              class="w-full bg-slate-600/60 border border-slate-500/50 rounded-lg pl-7 pr-7 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-400/70 focus:ring-1 focus:ring-indigo-400/30"
            />
            <button
              v-if="extraFieldsSearch"
              @click="extraFieldsSearch = ''"
              class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200 text-xs"
            >
              <i class="fa-solid fa-xmark"></i>
            </button>
          </div>
          <!-- Properties grid -->
          <div v-if="filteredExtraFields.length > 0" class="space-y-0 overflow-x-auto">
            <div
              v-for="[key, val] in filteredExtraFields"
              :key="key"
              class="grid grid-cols-[1fr_2fr_auto] gap-2 text-xs border-b border-slate-600/50 py-1 hover:bg-slate-600/30 group"
            >
              <span class="text-slate-400 font-mono break-all" v-html="highlightText(String(key), extraFieldsSearch)"></span>
              <span class="text-slate-200 font-mono break-all" v-html="highlightText(String(val), extraFieldsSearch)"></span>
              <button
                @click="filterByColumn(String(key), String(val))"
                class="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-indigo-400 transition-all shrink-0 px-0.5"
                :title="`Filter by ${key}: ${String(val).substring(0, 50)}`"
              >
                <i class="fa-solid fa-filter text-[10px]"></i>
              </button>
            </div>
          </div>
          <div v-else class="text-xs text-slate-500 text-center py-3">
            No properties match "{{ extraFieldsSearch }}"
          </div>
        </section>

        <!-- Source -->
        <section>
          <h4 class="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1.5">Source</h4>
          <p class="text-xs text-slate-500 truncate" :title="state.selectedEvent.sourceFile">
            {{ state.selectedEvent.sourceFile.split(/[/\\]/).pop() }}
          </p>
        </section>
      </div>
    </template>

    <!-- Empty state -->
    <div v-else class="flex flex-col items-center justify-center h-full text-slate-500">
      <i class="fa-solid fa-arrow-pointer text-3xl mb-3 text-slate-600"></i>
      <p class="text-sm">Select an event</p>
      <p class="text-xs mt-1">Click a row to see details</p>
    </div>
  </div>
</template>
