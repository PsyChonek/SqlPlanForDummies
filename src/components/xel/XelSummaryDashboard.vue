<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import * as d3 from 'd3';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import type { XelSessionStats, XelProblemStats, TimelineBucket } from '../../types/xel';
import { formatDuration, formatNumber } from '../../types/xel';

const { state, setFilter, clearFilter, setActiveView } = useXelState();

const stats = ref<XelSessionStats | null>(null);
const problems = ref<XelProblemStats | null>(null);
const buckets = ref<TimelineBucket[]>([]);
const eventChartRef = ref<HTMLDivElement | null>(null);
const waitChartRef = ref<HTMLDivElement | null>(null);
const loading = ref(false);

const fetchData = async () => {
  loading.value = true;
  try {
    const filter = { ...state.filter };
    const [s, p, t] = await Promise.all([
      xelApi.getStats(filter),
      xelApi.getProblemStats(filter),
      xelApi.getTimeline({ filter, bucketCount: 60 }),
    ]);
    stats.value = s;
    problems.value = p;
    buckets.value = t;
    renderCharts();
  } catch (err) {
    console.error('Dashboard data fetch failed:', err);
  } finally {
    loading.value = false;
  }
};

const renderCharts = () => {
  renderEventTimeline();
  renderWaitDistribution();
};

const renderEventTimeline = () => {
  if (!eventChartRef.value || buckets.value.length === 0) return;
  const el = eventChartRef.value;
  d3.select(el).selectAll('*').remove();

  const margin = { top: 10, right: 10, bottom: 25, left: 40 };
  const width = el.clientWidth;
  const height = 140;

  const svg = d3.select(el).append('svg').attr('width', width).attr('height', height);
  const g = svg.append('g');

  const x = d3.scaleTime()
    .domain([new Date(buckets.value[0].bucketStart), new Date(buckets.value[buckets.value.length - 1].bucketEnd)])
    .range([margin.left, width - margin.right]);

  const maxCount = d3.max(buckets.value, d => d.eventCount) || 1;
  const y = d3.scaleLinear()
    .domain([0, maxCount])
    .range([height - margin.bottom, margin.top]);

  const area = d3.area<TimelineBucket>()
    .x(d => x(new Date(d.bucketStart)))
    .y0(height - margin.bottom)
    .y1(d => y(d.eventCount))
    .curve(d3.curveMonotoneX);

  g.append('path').datum(buckets.value).attr('d', area).attr('fill', '#6366f1').attr('opacity', 0.3);

  const line = d3.line<TimelineBucket>()
    .x(d => x(new Date(d.bucketStart)))
    .y(d => y(d.eventCount))
    .curve(d3.curveMonotoneX);

  g.append('path').datum(buckets.value).attr('d', line).attr('fill', 'none').attr('stroke', '#818cf8').attr('stroke-width', 1.5);

  g.append('g')
    .attr('transform', `translate(0,${height - margin.bottom})`)
    .call(d3.axisBottom(x).ticks(6).tickFormat(d3.timeFormat('%H:%M') as any))
    .selectAll('text').attr('fill', '#94a3b8').style('font-size', '9px');

  g.append('g')
    .attr('transform', `translate(${margin.left},0)`)
    .call(d3.axisLeft(y).ticks(4))
    .selectAll('text').attr('fill', '#94a3b8').style('font-size', '9px');

  g.selectAll('.domain, .tick line').attr('stroke', '#334155');
};

const renderWaitDistribution = () => {
  if (!waitChartRef.value || !stats.value) return;
  const el = waitChartRef.value;
  d3.select(el).selectAll('*').remove();

  const eventCounts = Object.entries(stats.value.eventTypeCounts)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 8);

  if (eventCounts.length === 0) return;

  const width = el.clientWidth;
  const height = 140;
  const margin = { top: 5, right: 10, bottom: 5, left: 120 };

  const svg = d3.select(el).append('svg').attr('width', width).attr('height', height);

  const y = d3.scaleBand()
    .domain(eventCounts.map(d => d[0]))
    .range([margin.top, height - margin.bottom])
    .padding(0.2);

  const x = d3.scaleLinear()
    .domain([0, d3.max(eventCounts, d => d[1]) || 1])
    .range([margin.left, width - margin.right]);

  const colors = d3.scaleOrdinal(d3.schemeTableau10);

  svg.selectAll('rect')
    .data(eventCounts)
    .join('rect')
    .attr('x', margin.left)
    .attr('y', d => y(d[0])!)
    .attr('width', d => x(d[1]) - margin.left)
    .attr('height', y.bandwidth())
    .attr('fill', (_, i) => colors(i.toString()))
    .attr('rx', 2)
    .attr('opacity', 0.8)
    .style('cursor', 'pointer')
    .on('click', (_, d) => { setFilter({ eventNames: [d[0]] }); setActiveView('table'); });

  svg.selectAll('.label')
    .data(eventCounts)
    .join('text')
    .attr('x', margin.left - 4)
    .attr('y', d => y(d[0])! + y.bandwidth() / 2)
    .attr('text-anchor', 'end').attr('dominant-baseline', 'middle')
    .attr('fill', '#94a3b8').style('font-size', '9px')
    .text(d => d[0].replace(/_/g, ' '));

  svg.selectAll('.count')
    .data(eventCounts)
    .join('text')
    .attr('x', d => x(d[1]) + 4)
    .attr('y', d => y(d[0])! + y.bandwidth() / 2)
    .attr('dominant-baseline', 'middle')
    .attr('fill', '#e2e8f0').style('font-size', '9px')
    .text(d => d[1].toLocaleString());
};

const filterErrors = () => { setFilter({ errorsOnly: true }); setActiveView('table'); };
const filterDeadlocks = () => { setFilter({ eventNames: ['xml_deadlock_report', 'database_xml_deadlock_report', 'lock_deadlock', 'lock_deadlock_chain'] }); setActiveView('table'); };
const filterBlocked = () => { setFilter({ eventNames: ['blocked_process_report'] }); setActiveView('table'); };
const filterSession = (sid: number) => { clearFilter(); setFilter({ textSearch: `session_id:${sid}` }); setActiveView('table'); };

const categoryColor = (cat: string) => {
  const c: Record<string, string> = { io: 'text-blue-400', lock: 'text-red-400', latch: 'text-amber-400', network: 'text-purple-400', cpu: 'text-cyan-400', memory: 'text-pink-400', idle: 'text-slate-500', other: 'text-slate-400' };
  return c[cat] || 'text-slate-400';
};

const categoryDot = (cat: string) => {
  const c: Record<string, string> = { io: 'bg-blue-400', lock: 'bg-red-400', latch: 'bg-amber-400', network: 'bg-purple-400', cpu: 'bg-cyan-400', memory: 'bg-pink-400', idle: 'bg-slate-500', other: 'bg-slate-400' };
  return c[cat] || 'bg-slate-400';
};

watch(() => state.revision, fetchData);
watch(() => state.filter, fetchData, { deep: true });
watch(() => state.activeView, (v) => { if (v === 'dashboard') fetchData(); });
onMounted(fetchData);
</script>

<template>
  <div class="h-full overflow-auto p-4 relative">
    <!-- Loading overlay -->
    <div v-if="loading" class="absolute inset-0 z-20 flex items-center justify-center bg-slate-800/70 pointer-events-none">
      <div class="flex flex-col items-center gap-2">
        <i class="fa-solid fa-spinner fa-spin text-2xl text-indigo-400"></i>
        <span class="text-xs text-slate-300">Loading dashboard...</span>
      </div>
    </div>

    <div v-if="!stats" class="flex items-center justify-center h-full text-slate-500">
      <i class="fa-solid fa-spinner fa-spin text-xl"></i>
    </div>

    <div v-else class="space-y-4">
      <!-- Summary cards -->
      <div class="grid grid-cols-4 gap-3">
        <div class="bg-slate-700/50 rounded-xl px-4 py-3">
          <p class="text-xs text-slate-500 uppercase tracking-wider">Total Events</p>
          <p class="text-xl font-bold text-slate-200 mt-1">{{ stats.totalEvents.toLocaleString() }}</p>
        </div>
        <div class="bg-slate-700/50 rounded-xl px-4 py-3">
          <p class="text-xs text-slate-500 uppercase tracking-wider">Sessions</p>
          <p class="text-xl font-bold text-slate-200 mt-1">{{ stats.uniqueSessions.length }}</p>
        </div>
        <div class="bg-slate-700/50 rounded-xl px-4 py-3">
          <p class="text-xs text-slate-500 uppercase tracking-wider">Databases</p>
          <p class="text-xl font-bold text-slate-200 mt-1">{{ stats.uniqueDatabases.length }}</p>
        </div>
        <div class="bg-slate-700/50 rounded-xl px-4 py-3">
          <p class="text-xs text-slate-500 uppercase tracking-wider">Files</p>
          <p class="text-xl font-bold text-slate-200 mt-1">{{ stats.filesLoaded.length }}</p>
        </div>
      </div>

      <!-- Problem cards -->
      <div v-if="problems" class="grid grid-cols-5 gap-3">
        <button
          @click="filterErrors"
          class="bg-red-950/30 border border-red-800/30 rounded-xl px-4 py-3 text-left hover:bg-red-950/50 transition-colors"
          :class="{ 'opacity-40': problems.errorCount === 0 }"
        >
          <p class="text-xs text-red-400/70 uppercase tracking-wider">
            <i class="fa-solid fa-circle-exclamation mr-1"></i>Errors
          </p>
          <p class="text-xl font-bold text-red-400 mt-1">{{ problems.errorCount }}</p>
        </button>
        <button
          @click="filterDeadlocks"
          class="bg-red-950/30 border border-red-800/30 rounded-xl px-4 py-3 text-left hover:bg-red-950/50 transition-colors"
          :class="{ 'opacity-40': problems.deadlockCount === 0 }"
        >
          <p class="text-xs text-red-400/70 uppercase tracking-wider">
            <i class="fa-solid fa-skull-crossbones mr-1"></i>Deadlocks
          </p>
          <p class="text-xl font-bold text-red-400 mt-1">{{ problems.deadlockCount }}</p>
        </button>
        <button
          @click="filterBlocked"
          class="bg-yellow-950/30 border border-yellow-800/30 rounded-xl px-4 py-3 text-left hover:bg-yellow-950/50 transition-colors"
          :class="{ 'opacity-40': problems.blockedProcessCount === 0 }"
        >
          <p class="text-xs text-yellow-400/70 uppercase tracking-wider">
            <i class="fa-solid fa-ban mr-1"></i>Blocked
          </p>
          <p class="text-xl font-bold text-yellow-400 mt-1">{{ problems.blockedProcessCount }}</p>
        </button>
        <div
          class="bg-amber-950/20 border border-amber-800/20 rounded-xl px-4 py-3"
          :class="{ 'opacity-40': problems.lockWaitCount === 0 }"
        >
          <p class="text-xs text-amber-400/70 uppercase tracking-wider">
            <i class="fa-solid fa-lock mr-1"></i>Lock Waits
          </p>
          <p class="text-xl font-bold text-amber-400 mt-1">{{ problems.lockWaitCount }}</p>
        </div>
      </div>

      <!-- Charts + Top Waits row -->
      <div class="grid grid-cols-3 gap-4">
        <!-- Event timeline -->
        <div class="bg-slate-700/30 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Events Over Time</h3>
          <div ref="eventChartRef" class="w-full"></div>
        </div>

        <!-- Event type distribution -->
        <div class="bg-slate-700/30 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Event Types</h3>
          <div ref="waitChartRef" class="w-full"></div>
        </div>

        <!-- Top Wait Types -->
        <div v-if="problems && problems.topWaitTypes.length > 0" class="bg-slate-700/30 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Top Wait Types</h3>
          <div class="space-y-1 max-h-[140px] overflow-auto">
            <div
              v-for="wt in problems.topWaitTypes.slice(0, 10)"
              :key="wt.waitType"
              class="flex items-center gap-2 text-xs px-1.5 py-0.5"
            >
              <span class="w-1.5 h-1.5 rounded-full shrink-0" :class="categoryDot(wt.category)"></span>
              <span class="text-slate-400 font-mono text-[10px] truncate flex-1">{{ wt.waitType }}</span>
              <span class="text-slate-500 shrink-0 text-[10px]">x{{ wt.count }}</span>
              <span :class="categoryColor(wt.category)" class="shrink-0 font-medium text-[10px] tabular-nums w-14 text-right">{{ formatDuration(wt.totalDurationUs) }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Problem sessions + Top executions -->
      <div class="grid grid-cols-2 gap-4">
        <!-- Error sessions -->
        <div v-if="problems && problems.errorSessions.length > 0" class="bg-red-950/10 border border-red-800/20 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-red-400/70 uppercase tracking-wider mb-2">
            <i class="fa-solid fa-circle-exclamation mr-1"></i>Sessions with Errors
          </h3>
          <div class="space-y-1 max-h-48 overflow-auto">
            <button
              v-for="s in problems.errorSessions"
              :key="s.sessionId"
              @click="filterSession(s.sessionId)"
              class="w-full flex items-center gap-2 px-2 py-1 text-xs hover:bg-red-950/30 rounded text-left transition-colors"
            >
              <span class="text-red-400 font-medium w-10 shrink-0">S{{ s.sessionId }}</span>
              <span class="text-red-300 w-8 shrink-0 text-right">{{ s.count }}x</span>
              <span class="text-slate-400 truncate">{{ s.sampleObjectName || s.sampleEventName }}</span>
              <span class="text-slate-500 ml-auto shrink-0">{{ formatDuration(s.totalDurationUs) }}</span>
            </button>
          </div>
        </div>

        <!-- Wait sessions -->
        <div v-if="problems && problems.waitSessions.length > 0" class="bg-blue-950/10 border border-blue-800/20 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-blue-400/70 uppercase tracking-wider mb-2">
            <i class="fa-solid fa-hourglass-half mr-1"></i>Sessions with Most Waits
          </h3>
          <div class="space-y-1 max-h-48 overflow-auto">
            <button
              v-for="s in problems.waitSessions"
              :key="s.sessionId"
              @click="filterSession(s.sessionId)"
              class="w-full flex items-center gap-2 px-2 py-1 text-xs hover:bg-blue-950/30 rounded text-left transition-colors"
            >
              <span class="text-blue-400 font-medium w-10 shrink-0">S{{ s.sessionId }}</span>
              <span class="text-blue-300 w-8 shrink-0 text-right">{{ s.count }}x</span>
              <span class="text-slate-400 truncate">{{ s.sampleObjectName || s.sampleEventName }}</span>
              <span class="text-slate-500 ml-auto shrink-0">{{ formatDuration(s.totalDurationUs) }}</span>
            </button>
          </div>
        </div>
      </div>

      <!-- Top by duration / reads -->
      <div class="grid grid-cols-2 gap-4">
        <div class="bg-slate-700/30 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Slowest Executions</h3>
          <div class="space-y-1 max-h-48 overflow-auto">
            <div
              v-for="item in stats.topByDuration.slice(0, 10)"
              :key="item.id"
              class="flex items-center gap-2 px-2 py-1 text-xs hover:bg-slate-700/50 rounded cursor-pointer"
              @click="setActiveView('table')"
            >
              <span class="text-orange-400 font-medium w-16 shrink-0 text-right">{{ formatDuration(item.durationUs) }}</span>
              <span class="text-slate-400 w-12 shrink-0">S{{ item.sessionId }}</span>
              <span class="text-slate-300 truncate">{{ item.statementPreview || item.eventName }}</span>
            </div>
          </div>
        </div>
        <div class="bg-slate-700/30 rounded-xl p-3">
          <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Most Reads</h3>
          <div class="space-y-1 max-h-48 overflow-auto">
            <div
              v-for="item in stats.topByReads.slice(0, 10)"
              :key="item.id"
              class="flex items-center gap-2 px-2 py-1 text-xs hover:bg-slate-700/50 rounded cursor-pointer"
              @click="setActiveView('table')"
            >
              <span class="text-blue-400 font-medium w-16 shrink-0 text-right">{{ formatNumber(item.logicalReads) }}</span>
              <span class="text-slate-400 w-12 shrink-0">S{{ item.sessionId }}</span>
              <span class="text-slate-300 truncate">{{ item.statementPreview || item.eventName }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
