<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import CollapsiblePanel from '../CollapsiblePanel.vue';
import {
  getEventSeverity, getEventSeverityColor, getEventIcon,
  formatDuration, formatNumber, formatTimestampFull,
  getLockModeDescription,
} from '../../types/xel';
import type { BlockingAnalysis, ParsedDeadlockGraph, TransactionObject, XelEvent } from '../../types/xel';

const { state, selectEvent, setFilter, clearFilter, setActiveView } = useXelState();

const spidForProcess = (dl: ParsedDeadlockGraph, processId: string): string => {
  const proc = dl.processes.find(p => p.id === processId);
  return proc?.spid ? `S${proc.spid}` : processId.replace(/^process/, '').slice(0, 8);
};

interface GroupedResource {
  resourceType: string;
  objectName: string | null;
  indexName: string | null;
  count: number;
  holders: { label: string; mode: string }[];
  waiters: { label: string; mode: string }[];
}

const groupResources = (dl: ParsedDeadlockGraph): GroupedResource[] => {
  const map = new Map<string, GroupedResource>();
  for (const res of dl.resources) {
    if (res.resourceType === 'exchangeEvent') continue;
    const key = `${res.resourceType}|${res.objectName ?? ''}|${res.indexName ?? ''}`;
    let group = map.get(key);
    if (!group) {
      group = {
        resourceType: res.resourceType,
        objectName: res.objectName,
        indexName: res.indexName,
        count: 0,
        holders: [],
        waiters: [],
      };
      map.set(key, group);
    }
    group.count++;
    for (const h of res.holders) {
      const label = spidForProcess(dl, h.processId);
      const mode = h.mode ?? '?';
      if (!group.holders.some(x => x.label === label && x.mode === mode)) {
        group.holders.push({ label, mode });
      }
    }
    for (const w of res.waiters) {
      const label = spidForProcess(dl, w.processId);
      const mode = w.mode ?? '?';
      if (!group.waiters.some(x => x.label === label && x.mode === mode)) {
        group.waiters.push({ label, mode });
      }
    }
  }
  return Array.from(map.values());
};

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
  let filterValue = value;
  if (column === 'attach_activity_id' || column === 'attach_activity_id_xfer') {
    filterValue = value.split(':')[0];
  }
  clearFilter();
  setFilter({ textSearch: `${column}:${filterValue}` });
};

const findDeadlockRpc = (dl: ParsedDeadlockGraph) => {
  // 1. RPC filter: session + procedure + rpc_completed
  const spids = [...new Set(dl.processes.map(p => p.spid).filter((s): s is number => s !== null))];
  const procNames = [...new Set(
    dl.processes.flatMap(p =>
      p.executionStack
        .map(f => f.procName)
        .filter((n): n is string => n !== null)
        .map(n => {
          const dot = n.lastIndexOf('.');
          return dot >= 0 ? n.slice(dot + 1) : n;
        })
    )
  )];

  const sessionParts = spids.map(s => `session_id:${s}`);
  const procParts = procNames.map(n => `object_name:${n}`);

  const rpcParts: string[] = [];
  if (sessionParts.length > 1) {
    rpcParts.push(`(${sessionParts.join(' || ')})`);
  } else if (sessionParts.length === 1) {
    rpcParts.push(sessionParts[0]);
  }
  if (procParts.length > 1) {
    rpcParts.push(`(${procParts.join(' || ')})`);
  } else if (procParts.length === 1) {
    rpcParts.push(procParts[0]);
  }
  rpcParts.push('event_name:rpc_completed');
  const rpcFilter = rpcParts.join(' ');

  // 2. xactid from deadlock processes (matches lock events)
  const xactParts = [...new Set(dl.processes.map(p => p.xactId).filter((x): x is string => x !== null))]
    .map(x => `transaction_id:${x}`);

  // 3. deadlock report event itself (by event ID AND event type)
  const reportFilter = `(event_id:${dl.eventId} event_name:deadlock_report)`;

  // Combine: (rpc filter) || xactids || deadlock report
  // No time window — RPC calls may complete long before the deadlock report,
  // and the text search (session + event_name + transaction_id) already scopes results
  const orExtras = [...xactParts, reportFilter];
  const search = `(${rpcFilter}) || ${orExtras.join(' || ')}`;

  clearFilter();
  setFilter({ textSearch: search });
  setActiveView('table');
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

const diagnosisIcon = (d: string): string => {
  if (d === 'deadlock' || d === 'likely_deadlock') return 'fa-skull-crossbones';
  if (d === 'io_starvation') return 'fa-hard-drive';
  if (d.startsWith('lock')) return 'fa-lock';
  if (d === 'latch_contention') return 'fa-bolt';
  if (d === 'network_bottleneck') return 'fa-network-wired';
  if (d === 'memory_pressure') return 'fa-memory';
  if (d === 'cpu_pressure') return 'fa-microchip';
  return 'fa-question-circle';
};

const diagnosisIconColor = (d: string): string => {
  if (d === 'deadlock' || d === 'likely_deadlock') return 'text-red-400';
  if (d === 'io_starvation') return 'text-blue-400';
  if (d.startsWith('lock')) return 'text-red-400';
  if (d === 'latch_contention') return 'text-amber-400';
  if (d === 'network_bottleneck') return 'text-purple-400';
  if (d === 'memory_pressure') return 'text-pink-400';
  if (d === 'cpu_pressure') return 'text-cyan-400';
  return 'text-slate-400';
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
      <div class="shrink-0 px-3 py-2.5 bg-slate-700 border-b border-slate-600">
        <div class="flex items-center gap-2">
          <i
            :class="['fa-solid', getEventIcon(state.selectedEvent.eventName)]"
            :style="{ color: getEventSeverityColor(getEventSeverity(state.selectedEvent)) }"
            class="text-sm"
          ></i>
          <h3 class="text-sm font-semibold text-slate-200 truncate flex-1">
            {{ state.selectedEvent.eventName }}
          </h3>
          <button
            @click="copyText(buildLlmPrompt(), 'llm')"
            class="shrink-0 flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors"
            :class="copied === 'llm' ? 'bg-green-600/30 text-green-300' : 'bg-indigo-600/30 text-indigo-300 hover:bg-indigo-600/50 hover:text-indigo-200'"
            title="Copy all event details formatted for LLM analysis"
          >
            <i :class="copied === 'llm' ? 'fa-solid fa-check' : 'fa-solid fa-robot'" class="text-[10px]"></i>
            {{ copied === 'llm' ? 'Copied!' : 'AI' }}
          </button>
        </div>
        <!-- Compact meta row -->
        <div class="flex items-center gap-2 mt-1 text-[10px] text-slate-400 flex-wrap">
          <span>{{ formatTimestampFull(state.selectedEvent.timestamp) }}</span>
          <span v-if="state.selectedEvent.sessionId !== null" class="text-slate-500">S{{ state.selectedEvent.sessionId }}</span>
          <span v-if="state.selectedEvent.databaseName" class="text-slate-500 truncate">{{ state.selectedEvent.databaseName }}</span>
        </div>
      </div>

      <!-- Metrics -->
      <div class="flex-1 overflow-auto px-3 py-2 space-y-2">
        <!-- Blocking Analysis (auto-loaded) -->
        <div v-if="isBlockingRelated(state.selectedEvent)">
          <div v-if="analysisLoading" class="flex items-center gap-2 text-xs text-slate-400 py-1">
            <i class="fa-solid fa-spinner fa-spin text-[10px]"></i>
            Analyzing blocking...
          </div>

          <!-- Analysis Results -->
          <div v-if="showAnalysis && analysis" class="space-y-2">
            <CollapsiblePanel title="Blocking Analysis" icon="fa-link" icon-color="text-indigo-400" header-class="text-xs uppercase tracking-wider">
              <template #header-right>
                <select
                  v-model.number="analysisWindow"
                  @change="loadAnalysis"
                  @click.stop
                  class="bg-slate-700 text-slate-300 border border-slate-600 rounded px-1 py-0.5 outline-none text-xs"
                >
                  <option :value="15">15s</option>
                  <option :value="30">30s</option>
                  <option :value="60">1min</option>
                  <option :value="120">2min</option>
                  <option :value="300">5min</option>
                </select>
                <button
                  @click.stop="showAnalysis = false; analysis = null"
                  class="text-xs text-slate-500 hover:text-slate-300 px-1"
                >
                  <i class="fa-solid fa-xmark"></i>
                </button>
              </template>

              <!-- Summary + Diagnosis -->
              <div class="bg-slate-900/50 rounded-lg px-3 py-2">
                <div v-if="analysis.diagnosis && analysis.diagnosis !== 'no_waits'" class="flex items-center gap-1.5 mb-1.5">
                  <span
                    class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold"
                    :class="{
                      'bg-red-500/20 text-red-400': analysis.diagnosis === 'deadlock' || analysis.diagnosis === 'likely_deadlock' || analysis.diagnosis.startsWith('lock'),
                      'bg-blue-500/20 text-blue-400': analysis.diagnosis === 'io_starvation',
                      'bg-amber-500/20 text-amber-400': analysis.diagnosis === 'latch_contention',
                      'bg-purple-500/20 text-purple-400': analysis.diagnosis === 'network_bottleneck',
                      'bg-pink-500/20 text-pink-400': analysis.diagnosis === 'memory_pressure',
                      'bg-cyan-500/20 text-cyan-400': analysis.diagnosis === 'cpu_pressure',
                      'bg-slate-600/50 text-slate-400': !['deadlock','likely_deadlock','io_starvation','latch_contention','network_bottleneck','memory_pressure','cpu_pressure'].includes(analysis.diagnosis) && !analysis.diagnosis.startsWith('lock'),
                    }"
                  >
                    <i :class="['fa-solid', diagnosisIcon(analysis.diagnosis)]" class="text-[9px]"></i>
                    {{ diagnosisLabel(analysis.diagnosis) }}
                  </span>
                </div>
                <p class="text-xs text-slate-300 leading-relaxed">{{ analysis.summary }}</p>
              </div>

            <!-- Deadlock Graphs -->
            <CollapsiblePanel v-if="analysis.deadlocks.length > 0" :title="'Deadlock' + (analysis.deadlocks.length > 1 ? 's' : '')" icon="fa-skull-crossbones" icon-color="text-red-400" :badge="analysis.deadlocks.length" header-class="text-xs uppercase tracking-wider">
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
                        <span v-if="proc.ecid && proc.ecid > 0" class="text-slate-500 text-[10px]">({{ proc.ecid }} parallel threads)</span>
                        <span v-if="proc.appName" class="text-slate-500 truncate text-[10px]">({{ proc.appName }})</span>
                      </div>
                      <div class="grid grid-cols-[5.5rem_1fr] gap-x-2 gap-y-0.5 text-slate-400 ml-5 text-[10px]">
                        <template v-if="proc.waitResource">
                          <span class="text-slate-500">Wait resource</span>
                          <span class="text-yellow-300 font-mono break-all">{{ proc.waitResource }}</span>
                        </template>
                        <template v-if="proc.lockMode">
                          <span class="text-slate-500">Lock mode</span>
                          <span class="text-yellow-300"><span class="font-semibold">{{ proc.lockMode }}</span><span v-if="getLockModeDescription(proc.lockMode)" class="text-yellow-300/50 ml-1">{{ getLockModeDescription(proc.lockMode) }}</span></span>
                        </template>
                        <template v-if="proc.isolationLevel">
                          <span class="text-slate-500">Isolation</span>
                          <span class="text-slate-300">{{ proc.isolationLevel }}</span>
                        </template>
                        <template v-if="proc.waitTimeMs">
                          <span class="text-slate-500">Wait time</span>
                          <span class="text-slate-300">{{ proc.waitTimeMs.toLocaleString() }}ms</span>
                        </template>
                        <template v-if="proc.transactionName">
                          <span class="text-slate-500">Transaction</span>
                          <span class="text-slate-300">{{ proc.transactionName }}</span>
                        </template>
                        <template v-if="proc.tranCount">
                          <span class="text-slate-500">Tran count</span>
                          <span class="text-slate-300">{{ proc.tranCount }}</span>
                        </template>
                        <template v-if="proc.databaseName">
                          <span class="text-slate-500">Database</span>
                          <span class="text-slate-300">{{ proc.databaseName }}</span>
                        </template>
                        <template v-if="proc.hostname">
                          <span class="text-slate-500">Host</span>
                          <span class="text-slate-300">{{ proc.hostname }}</span>
                        </template>
                        <template v-if="proc.loginName">
                          <span class="text-slate-500">Login</span>
                          <span class="text-slate-300">{{ proc.loginName }}</span>
                        </template>
                      </div>
                      <!-- Execution Stack -->
                      <div v-if="proc.executionStack?.length > 0" class="mt-1 ml-5">
                        <span class="text-slate-500 text-[10px]">Execution stack:</span>
                        <div v-for="(frame, fi) in proc.executionStack" :key="fi"
                          class="text-[10px] font-mono bg-slate-900/40 px-2 py-0.5 rounded mt-0.5">
                          <div class="flex flex-wrap gap-x-2">
                            <span v-if="frame.procName" class="text-indigo-300">{{ frame.procName }}</span>
                            <span v-if="frame.line" class="text-slate-500">line {{ frame.line }}</span>
                            <span v-if="frame.queryHash" class="text-slate-500">hash: {{ frame.queryHash }}</span>
                            <span v-if="frame.queryPlanHash" class="text-slate-500">plan: {{ frame.queryPlanHash }}</span>
                          </div>
                          <div v-if="frame.sqlText" class="text-slate-400 mt-0.5 whitespace-pre-wrap break-all">{{ frame.sqlText }}</div>
                        </div>
                      </div>
                      <div v-if="proc.inputBuffer" class="mt-1 ml-5">
                        <span class="text-slate-500 text-[10px]">Input buffer:</span>
                        <pre class="text-[10px] font-mono px-2 py-1 rounded overflow-auto max-h-16 whitespace-pre-wrap break-all"
                          :class="proc.isVictim ? 'text-red-300/70 bg-red-950/40' : 'text-amber-300/70 bg-amber-950/30'"
                        >{{ proc.inputBuffer }}</pre>
                      </div>
                      <div v-if="proc.spid || proc.xactId" class="mt-1 ml-5 flex flex-wrap gap-2 items-center">
                        <button v-if="proc.spid"
                          @click="filterBySession(proc.spid!)"
                          class="text-[10px] text-indigo-400 hover:text-indigo-300"
                        >
                          <i class="fa-solid fa-filter mr-0.5"></i>Session {{ proc.spid }}
                        </button>
                        <button v-if="proc.xactId"
                          @click="filterByColumn('transaction_id', proc.xactId!)"
                          class="text-[10px] text-emerald-400 hover:text-emerald-300"
                          title="Filter XE events by transaction_id (XE action) — matches lock events with this transaction"
                        >
                          <i class="fa-solid fa-filter mr-0.5"></i>xactid {{ proc.xactId }}
                        </button>
                      </div>
                    </div>
                  </div>

                  <!-- Resources (filter out exchangeEvent noise) -->
                  <div v-if="dl.resources.length > 0">
                    <div class="text-[10px] text-slate-500 font-semibold uppercase mb-0.5">Contended Resources</div>
                    <div class="space-y-0.5">
                      <div
                        v-for="(grp, gi) in groupResources(dl)"
                        :key="gi"
                        class="text-[10px] text-slate-400 bg-slate-800/50 rounded px-2 py-1"
                      >
                        <span class="text-yellow-300 font-medium">{{ grp.resourceType }}</span>
                        <span v-if="grp.objectName" class="text-slate-300 ml-1">{{ grp.objectName }}</span>
                        <span v-if="grp.indexName" class="text-slate-500 ml-1">({{ grp.indexName }})</span>
                        <span v-if="grp.count > 1" class="text-slate-500 ml-1">&times;{{ grp.count }}</span>
                        <div class="flex flex-wrap gap-2 mt-0.5">
                          <span v-for="(h, hi) in grp.holders" :key="'h-' + hi" class="text-green-400">
                            <span class="text-green-500/70">{{ h.label }}</span> holds: {{ h.mode }}
                          </span>
                          <span v-for="(w, wi) in grp.waiters" :key="'w-' + wi" class="text-red-400">
                            <span class="text-red-500/70">{{ w.label }}</span> waits: {{ w.mode }}
                          </span>
                        </div>
                      </div>
                      <div v-if="dl.resources.filter(r => r.resourceType === 'exchangeEvent').length > 0"
                        class="text-[10px] text-slate-500 italic"
                      >
                        + {{ dl.resources.filter(r => r.resourceType === 'exchangeEvent').length }} parallel exchange events (hidden)
                      </div>
                    </div>
                  </div>

                  <div class="flex flex-wrap gap-3">
                    <button
                      @click="jumpToEvent(dl.eventId)"
                      class="text-[10px] text-indigo-400 hover:text-indigo-300"
                    >
                      View deadlock event #{{ dl.eventId }}
                    </button>
                    <button
                      @click="findDeadlockRpc(dl)"
                      class="text-[10px] text-cyan-400 hover:text-cyan-300"
                    >
                      <i class="fa-solid fa-magnifying-glass mr-0.5"></i>Find RPC calls
                    </button>
                  </div>
                </div>
              </div>
            </CollapsiblePanel>

            <!-- Deadlock Lock Events (confirmed via deadlock_id) -->
            <CollapsiblePanel v-if="analysis.deadlockId && analysis.deadlockLockEvents.length > 0" title="Deadlock Lock Events" icon="fa-skull-crossbones" icon-color="text-red-400" :badge="analysis.deadlockLockEvents.length" header-class="text-xs uppercase tracking-wider">
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
                  <div class="grid grid-cols-[5.5rem_1fr] gap-x-2 gap-y-0.5 text-[10px] text-slate-400">
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
                      <span class="text-slate-300 font-mono break-all">{{ ev.resourceDescription }}</span>
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
            </CollapsiblePanel>

            <!-- Blocking Chain -->
            <CollapsiblePanel v-if="analysis.blockingChain.length > 0" title="Blocking Chain" icon="fa-link" icon-color="text-orange-400" :badge="analysis.blockingChain.length" header-class="text-xs uppercase tracking-wider">
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
                    <div class="flex items-center gap-1.5">
                      <button
                        v-if="link.xactId"
                        @click="filterByColumn('transaction_id', link.xactId)"
                        class="text-indigo-400 hover:text-indigo-300 text-[10px]"
                        :title="`Filter by transaction ${link.xactId}`"
                      >
                        <i class="fa-solid fa-filter"></i> TXN
                      </button>
                      <button
                        @click="filterBySession(link.sessionId)"
                        class="text-slate-500 hover:text-slate-300 text-[10px]"
                        title="Filter by session"
                      >
                        <i class="fa-solid fa-filter"></i> S{{ link.sessionId }}
                      </button>
                    </div>
                  </div>

                  <div class="grid grid-cols-[5.5rem_1fr] gap-x-2 gap-y-0.5 text-[10px] text-slate-400">
                    <template v-if="link.blockedBySession">
                      <span>Blocked by</span>
                      <span class="text-red-400">Session {{ link.blockedBySession }}</span>
                    </template>
                    <template v-if="link.waitResource">
                      <span>Resource</span>
                      <span class="text-yellow-300 font-mono break-all">{{ link.waitResource }}</span>
                    </template>
                    <template v-if="link.lockMode">
                      <span>Lock mode</span>
                      <span class="text-yellow-300" :title="getLockModeDescription(link.lockMode) ?? ''">{{ link.lockMode }}<span v-if="getLockModeDescription(link.lockMode)" class="text-yellow-300/50 text-[9px] ml-1">{{ getLockModeDescription(link.lockMode) }}</span></span>
                    </template>
                    <template v-if="link.status">
                      <span>Status</span>
                      <span :class="link.status === 'sleeping' ? 'text-amber-400' : link.status === 'running' ? 'text-green-400' : 'text-orange-400'">{{ link.status }}</span>
                    </template>
                    <template v-if="link.hostname">
                      <span>Host</span>
                      <span class="text-slate-300 truncate">{{ link.hostname }}</span>
                    </template>
                    <template v-if="link.appName">
                      <span>App</span>
                      <span class="text-slate-300 truncate">{{ link.appName }}</span>
                    </template>
                    <template v-if="link.username">
                      <span>User</span>
                      <span class="text-slate-300">{{ link.username }}</span>
                    </template>
                    <template v-if="link.isolationLevel">
                      <span>Isolation</span>
                      <span class="text-slate-300">{{ link.isolationLevel }}</span>
                    </template>
                    <template v-if="link.xactId">
                      <span>Transaction ID</span>
                      <button
                        @click="filterByColumn('transaction_id', link.xactId)"
                        class="text-indigo-400 hover:text-indigo-300 font-mono cursor-pointer"
                        :title="`Filter events by transaction_id:${link.xactId}`"
                      >{{ link.xactId }}</button>
                    </template>
                    <template v-if="link.tranCount">
                      <span>Tran count</span>
                      <span class="text-slate-300">{{ link.tranCount }}</span>
                    </template>
                    <template v-if="link.waitTimeMs">
                      <span>Wait time</span>
                      <span class="text-orange-300">{{ (link.waitTimeMs / 1000).toFixed(1) }}s</span>
                    </template>
                    <template v-if="link.lastBatchStarted">
                      <span>Last batch</span>
                      <span class="text-slate-300">{{ link.lastBatchStarted }}</span>
                    </template>
                  </div>

                  <!-- Execution Stack -->
                  <div v-if="link.executionStack?.some((f: any) => f.queryHash)" class="mt-1.5">
                    <template v-for="(frame, fi) in link.executionStack" :key="fi">
                      <div v-if="frame.queryHash" class="text-[10px] font-mono text-slate-400 bg-slate-900/40 px-2 py-0.5 rounded mt-0.5">
                        <span :class="link.role === 'root_blocker' ? 'text-red-300' : 'text-indigo-300'">{{ frame.queryHash }}</span>
                        <span v-if="frame.queryPlanHash" class="text-slate-500 ml-1">plan: {{ frame.queryPlanHash }}</span>
                        <span v-if="frame.line" class="text-slate-500 ml-1">line {{ frame.line }}</span>
                      </div>
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
            </CollapsiblePanel>

            <!-- Blocked Process Reports -->
            <CollapsiblePanel v-if="analysis.blockedProcessReports.length > 0" title="Blocked Process Reports" icon="fa-ban" icon-color="text-orange-400" :badge="analysis.blockedProcessReports.length" header-class="text-xs uppercase tracking-wider">
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
                  <div class="grid grid-cols-[5.5rem_1fr] gap-x-2 gap-y-0.5 text-[10px] text-slate-400">
                    <template v-if="bpr.blockedXactId">
                      <span>Victim TXN</span>
                      <button
                        @click="filterByColumn('transaction_id', bpr.blockedXactId!)"
                        class="text-indigo-400 hover:text-indigo-300 font-mono text-left cursor-pointer"
                        :title="`Filter by transaction_id:${bpr.blockedXactId}`"
                      >{{ bpr.blockedXactId }}</button>
                    </template>
                    <template v-if="bpr.blockingXactId">
                      <span>Blocker TXN</span>
                      <button
                        @click="filterByColumn('transaction_id', bpr.blockingXactId!)"
                        class="text-red-400 hover:text-red-300 font-mono text-left cursor-pointer"
                        :title="`Filter by transaction_id:${bpr.blockingXactId}`"
                      >{{ bpr.blockingXactId }}</button>
                    </template>
                    <template v-if="bpr.blockedWaitResource">
                      <span>Wait resource</span>
                      <span class="text-yellow-300 font-mono break-all">
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
            </CollapsiblePanel>

            <!-- Blocker Events -->
            <CollapsiblePanel v-if="analysis.blockerEvents.length > 0" title="Blocked Session Events" icon="fa-hand" icon-color="text-red-400" :badge="analysis.blockerEvents.length" header-class="text-xs uppercase tracking-wider">
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
            </CollapsiblePanel>

            <!-- Lock Escalations -->
            <CollapsiblePanel v-if="analysis.lockEscalations.length > 0" title="Lock Escalations" icon="fa-arrow-up" icon-color="text-amber-400" :badge="analysis.lockEscalations.length" header-class="text-xs uppercase tracking-wider">
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
            </CollapsiblePanel>

            <!-- Wait Stats (aggregated) -->
            <CollapsiblePanel v-if="analysis.waitStats.length > 0" title="Wait Statistics" icon="fa-clock" icon-color="text-slate-400" :badge="`${analysis.waitEvents.length} events`" header-class="text-xs uppercase tracking-wider">
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
            </CollapsiblePanel>

            <!-- Recommendations -->
            <CollapsiblePanel v-if="analysis.recommendations.length > 0" title="Recommendations" icon="fa-lightbulb" icon-color="text-amber-400" :badge="analysis.recommendations.length" header-class="text-xs uppercase tracking-wider">
              <ul class="space-y-1.5">
                <li
                  v-for="(rec, i) in analysis.recommendations"
                  :key="i"
                  class="text-xs text-slate-300 bg-slate-900/40 rounded px-2.5 py-1.5 leading-relaxed"
                >
                  {{ rec }}
                </li>
              </ul>
            </CollapsiblePanel>

            </CollapsiblePanel>
          </div>

          <!-- Analysis Error -->
          <div v-if="analysisError" class="text-xs text-red-400 mt-1">
            {{ analysisError }}
          </div>
        </div>

        <!-- Identity (only show if there's non-header info) -->
        <CollapsiblePanel v-if="state.selectedEvent.username || state.selectedEvent.clientAppName || state.selectedEvent.objectName" title="Identity" icon="fa-id-card" icon-color="text-slate-400" header-class="text-xs uppercase tracking-wider" :collapsed="true">
          <div class="grid grid-cols-[5rem_1fr] gap-x-3 gap-y-1 text-xs">
            <span class="text-slate-500">Event ID</span>
            <span class="text-slate-300">{{ state.selectedEvent.id }}</span>
            <template v-if="state.selectedEvent.username">
              <span class="text-slate-500">User</span>
              <span class="text-slate-300 truncate">{{ state.selectedEvent.username }}</span>
            </template>
            <template v-if="state.selectedEvent.clientAppName">
              <span class="text-slate-500">App</span>
              <span class="text-slate-300 truncate">{{ state.selectedEvent.clientAppName }}</span>
            </template>
            <template v-if="state.selectedEvent.objectName">
              <span class="text-slate-500">Object</span>
              <span class="text-slate-300 font-mono truncate">{{ state.selectedEvent.objectName }}</span>
            </template>
          </div>
        </CollapsiblePanel>

        <!-- Performance -->
        <CollapsiblePanel v-if="state.selectedEvent.durationUs !== null || state.selectedEvent.logicalReads !== null" title="Performance" icon="fa-gauge-high" icon-color="text-cyan-400" header-class="text-xs uppercase tracking-wider">
          <div class="grid grid-cols-[5rem_1fr] gap-x-3 gap-y-1 text-xs">
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
        </CollapsiblePanel>

        <!-- Lock Info -->
        <CollapsiblePanel v-if="state.selectedEvent.resourceType || state.selectedEvent.lockMode || state.selectedEvent.waitType || state.selectedEvent.extraFields['wait_resource']" title="Lock / Wait" icon="fa-lock" icon-color="text-yellow-400" header-class="text-xs uppercase tracking-wider">
          <div class="grid grid-cols-[5rem_1fr] gap-x-3 gap-y-1 text-xs">
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
        </CollapsiblePanel>

        <!-- Related Objects (from same session/transaction) -->
        <CollapsiblePanel v-if="txnObjectsLoading || txnObjects.length > 0" title="Related Objects" icon="fa-link" icon-color="text-emerald-400" :badge="txnObjects.length || ''" header-class="text-xs uppercase tracking-wider">
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
        </CollapsiblePanel>


        <!-- SQL Text -->
        <CollapsiblePanel v-if="state.selectedEvent.sqlText || state.selectedEvent.statement" title="SQL Text" icon="fa-code" icon-color="text-slate-400" header-class="text-xs uppercase tracking-wider">
          <template #header-right>
            <button
              @click.stop="copyText(state.selectedEvent!.statement || state.selectedEvent!.sqlText || '', 'sql')"
              class="text-xs px-1.5 py-0.5 rounded text-slate-500 hover:text-slate-200 hover:bg-slate-700 transition-colors"
            >
              <i :class="copied === 'sql' ? 'fa-solid fa-check text-green-400' : 'fa-regular fa-copy'"></i>
            </button>
          </template>
          <pre class="text-xs text-slate-300 font-mono bg-slate-900/50 px-3 py-2 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap break-all">{{ state.selectedEvent.statement || state.selectedEvent.sqlText }}</pre>
        </CollapsiblePanel>

        <!-- Deadlock Graph -->
        <CollapsiblePanel v-if="state.selectedEvent.deadlockGraph" title="Deadlock Graph" icon="fa-skull-crossbones" icon-color="text-red-400" header-class="text-xs uppercase tracking-wider">
          <template #header-right>
            <button
              @click.stop="copyText(state.selectedEvent!.deadlockGraph || '', 'deadlock')"
              class="text-xs px-1.5 py-0.5 rounded text-slate-500 hover:text-slate-200 hover:bg-slate-700 transition-colors"
            >
              <i :class="copied === 'deadlock' ? 'fa-solid fa-check text-green-400' : 'fa-regular fa-copy'"></i>
            </button>
          </template>
          <pre class="text-xs text-slate-400 font-mono bg-slate-900/50 px-3 py-2 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap break-all">{{ state.selectedEvent.deadlockGraph }}</pre>
        </CollapsiblePanel>

        <!-- Blocked Process Report -->
        <CollapsiblePanel v-if="state.selectedEvent.blockedProcessReport" title="Blocked Process Report (raw)" icon="fa-ban" icon-color="text-orange-400" header-class="text-xs uppercase tracking-wider">
          <template #header-right>
            <button
              @click.stop="copyText(state.selectedEvent!.blockedProcessReport || '', 'bpr')"
              class="text-xs px-1.5 py-0.5 rounded text-slate-500 hover:text-slate-200 hover:bg-slate-700 transition-colors"
            >
              <i :class="copied === 'bpr' ? 'fa-solid fa-check text-green-400' : 'fa-regular fa-copy'"></i>
            </button>
          </template>
          <pre class="text-xs text-slate-400 font-mono bg-slate-900/50 px-3 py-2 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap break-all">{{ state.selectedEvent.blockedProcessReport }}</pre>
        </CollapsiblePanel>

        <!-- Extra Fields -->
        <CollapsiblePanel v-if="Object.keys(state.selectedEvent.extraFields).length > 0" title="Additional Fields" icon="fa-list-ul" icon-color="text-indigo-400" :badge="Object.keys(state.selectedEvent.extraFields).length" :collapsed="true" header-class="text-xs uppercase tracking-wider">
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
        </CollapsiblePanel>

        <!-- Source (inline) -->
        <div class="text-[10px] text-slate-500 truncate pt-1" :title="state.selectedEvent.sourceFile">
          <i class="fa-solid fa-file mr-1"></i>{{ state.selectedEvent.sourceFile.split(/[/\\]/).pop() }}
        </div>
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
