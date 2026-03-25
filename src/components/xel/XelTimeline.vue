<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue';
import * as d3 from 'd3';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import type { XelEvent } from '../../types/xel';
import { getEventSeverityColor, getEventSeverity, formatDuration, formatTimestamp, getLockModeDescription } from '../../types/xel';

const { state, selectEvent } = useXelState();

const tooltip = ref<{ x: number; y: number; event: XelEvent } | null>(null);

const viewport = ref<HTMLDivElement>();
let resizeObserver: ResizeObserver | null = null;

const timelineEvents = ref<XelEvent[]>([]);
const loading = ref(false);
const mode = ref<'related' | 'filtered'>('related');
const timeWindow = ref(30); // seconds

const hasSelection = computed(() => state.selectedEvent !== null);

let fetchVersion = 0;
let debounceHandle: ReturnType<typeof setTimeout> | null = null;

const scheduleRefresh = () => {
  if (debounceHandle) clearTimeout(debounceHandle);
  debounceHandle = setTimeout(fetchTimelineData, 50);
};

const fetchTimelineData = async () => {
  const version = ++fetchVersion;
  loading.value = true;
  tooltip.value = null;
  // Clear old SVG immediately so stale data doesn't persist across mode switches
  if (viewport.value) d3.select(viewport.value).selectAll('*').remove();

  try {
    const windowMs = timeWindow.value * 1000;
    const sel = state.selectedEvent;

    // Both modes require a selected event as the time anchor
    if (!sel) {
      if (version === fetchVersion) {
        timelineEvents.value = [];
      }
      return;
    }

    const selStart = new Date(sel.timestamp).getTime();
    const selDur = sel.durationUs ? sel.durationUs / 1000 : 0;
    const selEnd = selStart + selDur;

    let events: XelEvent[];

    if (mode.value === 'related') {
      events = await xelApi.getRelatedEvents(
        sel.id,
        windowMs,
        3000,
      );
    } else {
      const filter = { ...state.filter };
      filter.timeFrom = new Date(selStart - windowMs).toISOString();
      filter.timeTo = new Date(selEnd + windowMs).toISOString();
      const response = await xelApi.queryEvents({
        filter,
        offset: 0,
        limit: 3000,
        sortBy: 'timestamp',
        sortDesc: false,
      });
      events = response.events;
    }

    // Discard if a newer fetch was started
    if (version !== fetchVersion) return;

    timelineEvents.value = events;
    renderTimeline();
  } catch (err) {
    if (version === fetchVersion) {
      console.error('Failed to fetch timeline data:', err);
    }
  } finally {
    if (version === fetchVersion) {
      loading.value = false;
    }
  }
};

let currentZoom: d3.ZoomBehavior<SVGSVGElement, unknown> | null = null;
let currentSvg: d3.Selection<SVGSVGElement, unknown, null, undefined> | null = null;

const zoomIn = () => { if (currentSvg && currentZoom) currentSvg.transition().duration(300).call(currentZoom.scaleBy, 1.3); };
const zoomOut = () => { if (currentSvg && currentZoom) currentSvg.transition().duration(300).call(currentZoom.scaleBy, 0.7); };
const zoomFit = () => {
  if (!currentSvg || !currentZoom || !viewport.value) return;
  const svgNode = currentSvg.select('g').node() as SVGGElement | null;
  if (!svgNode) return;
  const bounds = svgNode.getBBox();
  if (bounds.width === 0 || bounds.height === 0) return;
  const vw = viewport.value.clientWidth;
  const vh = viewport.value.clientHeight;
  const scale = Math.min(vw / (bounds.width + 40), vh / (bounds.height + 40));
  const tx = (vw - bounds.width * scale) / 2 - bounds.x * scale;
  const ty = (vh - bounds.height * scale) / 2 - bounds.y * scale;
  currentSvg.transition().duration(500).call(currentZoom.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
};

const renderTimeline = () => {
  if (!viewport.value) return;

  const container = viewport.value;

  // Always clear old SVG, even if there are no events to render
  d3.select(container).selectAll('*').remove();
  if (timelineEvents.value.length === 0) return;
  // Defer if container has no dimensions yet (tab not visible)
  if (container.clientWidth < 10 || container.clientHeight < 10) {
    requestAnimationFrame(renderTimeline);
    return;
  }

  const events = timelineEvents.value;
  const margin = { top: 35, right: 20, bottom: 30, left: 90 };
  const width = container.clientWidth;
  const LANE_GAP = 2;

  // Group by session
  const sessionMap = new Map<number, XelEvent[]>();
  for (const e of events) {
    const sid = e.sessionId ?? 0;
    if (!sessionMap.has(sid)) sessionMap.set(sid, []);
    sessionMap.get(sid)!.push(e);
  }

  const sessionIds = Array.from(sessionMap.keys()).sort((a, b) => a - b);

  // Compute tight time range including durations
  let tMin = Infinity, tMax = -Infinity;
  for (const e of events) {
    const ts = new Date(e.timestamp).getTime();
    const dur = e.durationUs ? e.durationUs / 1000 : 0;
    if (ts < tMin) tMin = ts;
    if (ts + dur > tMax) tMax = ts + dur;
  }
  // Add 5% padding on each side, minimum 1 second span
  const span = Math.max(tMax - tMin, 1000);
  const pad = span * 0.05;
  tMin -= pad;
  tMax += pad;

  const xScale = d3.scaleTime()
    .domain([new Date(tMin), new Date(tMax)])
    .range([margin.left, width - margin.right]);

  // Pack overlapping events into sub-lanes per session (pixel-based)
  const SUB_LANE_HEIGHT = 20;
  const SUB_LANE_GAP = 2;
  const MAX_SUB_LANES = 5;
  const MIN_BAR_PX = 3; // must match the minimum bar width used in rendering
  const sessionSubLanes = new Map<number, Map<XelEvent, number>>();
  const sessionSubLaneCounts = new Map<number, number>();

  for (const [sid, sessionEvents] of sessionMap.entries()) {
    // Sort by start time
    const sorted = [...sessionEvents].sort((a, b) =>
      new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
    );
    // Each sub-lane tracks its pixel end position
    const subLaneEndsPx: number[] = [];
    const eventLane = new Map<XelEvent, number>();

    for (const ev of sorted) {
      const start = new Date(ev.timestamp).getTime();
      const dur = ev.durationUs ? ev.durationUs / 1000 : 0;
      const xStart = xScale(start);
      const xEnd = xScale(start + dur);
      const barEnd = xStart + Math.max(MIN_BAR_PX, xEnd - xStart);

      // Find first sub-lane where this event fits (no pixel overlap)
      let placed = -1;
      for (let i = 0; i < subLaneEndsPx.length; i++) {
        if (xStart >= subLaneEndsPx[i]) {
          placed = i;
          subLaneEndsPx[i] = barEnd;
          break;
        }
      }
      if (placed === -1 && subLaneEndsPx.length < MAX_SUB_LANES) {
        placed = subLaneEndsPx.length;
        subLaneEndsPx.push(barEnd);
      } else if (placed === -1) {
        // Over max sub-lanes — stack onto least-full lane
        placed = 0;
        for (let i = 1; i < subLaneEndsPx.length; i++) {
          if (subLaneEndsPx[i] < subLaneEndsPx[placed]) placed = i;
        }
        subLaneEndsPx[placed] = barEnd;
      }
      eventLane.set(ev, placed);
    }
    sessionSubLanes.set(sid, eventLane);
    sessionSubLaneCounts.set(sid, Math.max(1, subLaneEndsPx.length));
  }

  // Compute Y offset for each session based on accumulated sub-lane heights
  const sessionYOffsets = new Map<number, number>();
  let currentY = margin.top;
  for (const sid of sessionIds) {
    sessionYOffsets.set(sid, currentY);
    const count = sessionSubLaneCounts.get(sid) ?? 1;
    currentY += count * (SUB_LANE_HEIGHT + SUB_LANE_GAP) + LANE_GAP;
  }
  const totalHeight = currentY - margin.top;

  const svg = d3.select(container)
    .append('svg')
    .attr('width', '100%')
    .attr('height', '100%')
    .style('display', 'block');

  currentSvg = svg as any;

  const g = svg.append('g');

  const zoom = d3.zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.05, 100])
    .on('zoom', (event) => { g.attr('transform', event.transform); tooltip.value = null; });
  currentZoom = zoom;
  svg.call(zoom);

  // Time axis
  g.append('g')
    .attr('transform', `translate(0, ${margin.top - 5})`)
    .call(d3.axisTop(xScale).ticks(8).tickFormat(d3.timeFormat('%H:%M:%S') as any))
    .selectAll('text').attr('fill', '#94a3b8').style('font-size', '10px');
  g.selectAll('.domain, .tick line').attr('stroke', '#475569');

  // Selected event time marker
  if (state.selectedEvent) {
    const selTime = new Date(state.selectedEvent.timestamp).getTime();
    const selX = xScale(selTime);
    g.append('line')
      .attr('x1', selX).attr('x2', selX)
      .attr('y1', margin.top).attr('y2', margin.top + totalHeight)
      .attr('stroke', '#818cf8').attr('stroke-width', 1.5)
      .attr('stroke-dasharray', '4,3').attr('opacity', 0.6);
  }

  // Session lanes
  for (const sid of sessionIds) {
    const sessionY = sessionYOffsets.get(sid)!;
    const subCount = sessionSubLaneCounts.get(sid) ?? 1;
    const laneHeight = subCount * (SUB_LANE_HEIGHT + SUB_LANE_GAP) - SUB_LANE_GAP;
    const isSelectedSession = state.selectedEvent?.sessionId === sid;

    // Lane background
    g.append('rect')
      .attr('x', margin.left - 2).attr('y', sessionY)
      .attr('width', width - margin.left - margin.right + 4)
      .attr('height', laneHeight)
      .attr('fill', isSelectedSession ? '#1e3a5f' : '#1e293b')
      .attr('rx', 2);

    // Session label
    g.append('text')
      .attr('x', margin.left - 6).attr('y', sessionY + laneHeight / 2)
      .attr('text-anchor', 'end').attr('dominant-baseline', 'middle')
      .attr('fill', isSelectedSession ? '#93c5fd' : '#94a3b8')
      .attr('font-weight', isSelectedSession ? 'bold' : 'normal')
      .style('font-size', '10px')
      .text(`S${sid}`);
  }

  // Events
  for (const [sid, sessionEvents] of sessionMap.entries()) {
    const sessionY = sessionYOffsets.get(sid)!;
    const eventLanes = sessionSubLanes.get(sid)!;

    for (const event of sessionEvents) {
      const subLane = eventLanes.get(event) ?? 0;
      const y = sessionY + subLane * (SUB_LANE_HEIGHT + SUB_LANE_GAP);
      const ts = new Date(event.timestamp).getTime();
      const x = xScale(ts);
      const dur = event.durationUs ? event.durationUs / 1000 : 0;
      const endX = xScale(ts + dur);
      const barWidth = Math.max(3, endX - x);

      const severity = getEventSeverity(event);
      const color = getEventSeverityColor(severity);
      const isSelected = state.selectedEvent?.id === event.id;
      const isError = event.result === 'Error' || event.result === 'Abort';
      const isDeadlock = event.eventName.includes('deadlock');
      const isBPR = event.eventName === 'blocked_process_report';
      const isLockEvent = event.eventName.startsWith('lock_');

      // Main bar
      g.append('rect')
        .attr('x', x).attr('y', y + 2)
        .attr('width', barWidth).attr('height', SUB_LANE_HEIGHT - 4)
        .attr('fill', color)
        .attr('opacity', isSelected ? 1 : 0.7)
        .attr('stroke', isSelected ? '#fff' : isError ? '#ef4444' : 'none')
        .attr('stroke-width', isSelected ? 2 : isError ? 1.5 : 0)
        .attr('rx', 2)
        .style('cursor', 'pointer')
        .on('click', function(mouseEvent: MouseEvent) {
          const rect = (viewport.value as HTMLElement).getBoundingClientRect();
          tooltip.value = {
            x: mouseEvent.clientX - rect.left + 10,
            y: mouseEvent.clientY - rect.top - 10,
            event,
          };
        })
        .on('mouseleave', () => { /* keep tooltip until click elsewhere */ });

      // CPU indicator — thin bottom stripe showing CPU fraction of total duration
      if (event.durationUs && event.cpuTimeUs && event.durationUs > event.cpuTimeUs * 2 && barWidth > 6) {
        const cpuFraction = Math.min(1, event.cpuTimeUs / event.durationUs);
        const stripeH = 3;
        // Green stripe = CPU portion, positioned at bottom of bar
        g.append('rect')
          .attr('x', x).attr('y', y + SUB_LANE_HEIGHT - 4 - stripeH + 2)
          .attr('width', Math.max(2, barWidth * cpuFraction)).attr('height', stripeH)
          .attr('fill', '#22c55e')
          .attr('opacity', 0.8)
          .attr('rx', 1)
          .style('pointer-events', 'none');
      }

      // Icon markers for special events
      if (isDeadlock || isBPR || isLockEvent) {
        const icon = isDeadlock ? '\u2620' : isBPR ? '\u26D4' : '\uD83D\uDD12';
        g.append('text')
          .attr('x', x + barWidth / 2).attr('y', y - 1)
          .attr('text-anchor', 'middle').attr('dominant-baseline', 'auto')
          .attr('fill', isDeadlock ? '#ef4444' : isBPR ? '#f97316' : '#eab308')
          .style('font-size', '9px').style('pointer-events', 'none')
          .text(icon);
      }

      // Error X marker
      if (isError && !isDeadlock && !isBPR) {
        g.append('text')
          .attr('x', x + barWidth / 2).attr('y', y - 1)
          .attr('text-anchor', 'middle').attr('dominant-baseline', 'auto')
          .attr('fill', '#ef4444')
          .style('font-size', '8px').style('font-weight', 'bold').style('pointer-events', 'none')
          .text('\u2716');
      }
    }
  }
};

watch(() => state.selectedEvent?.id, scheduleRefresh);
watch(() => state.revision, scheduleRefresh);
watch(timeWindow, scheduleRefresh);
watch(mode, scheduleRefresh);
// Refresh when tab becomes visible (v-show keeps component alive)
watch(() => state.activeView, (v) => { if (v === 'timeline') scheduleRefresh(); });

onMounted(() => {
  if (viewport.value) {
    resizeObserver = new ResizeObserver(() => {
      if (timelineEvents.value.length > 0) renderTimeline();
    });
    resizeObserver.observe(viewport.value);
  }
  fetchTimelineData();
});

onUnmounted(() => { resizeObserver?.disconnect(); });
</script>

<template>
  <div class="flex flex-col h-full relative">
    <!-- Controls -->
    <div class="absolute top-2 left-2 z-10 flex items-center gap-2">
      <div class="flex bg-slate-800/90 rounded-lg border border-slate-600 text-xs overflow-hidden">
        <button
          @click="mode = 'related'"
          class="px-2.5 py-1 transition-colors"
          :class="mode === 'related' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-slate-200'"
        >Related</button>
        <button
          @click="mode = 'filtered'"
          class="px-2.5 py-1 transition-colors"
          :class="mode === 'filtered' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-slate-200'"
        >All Filtered</button>
      </div>
      <div class="flex items-center gap-1 bg-slate-800/90 rounded-lg border border-slate-600 px-2 py-1 text-xs text-slate-400">
        <span>Window:</span>
        <select v-model.number="timeWindow" class="bg-slate-700 text-slate-300 border border-slate-600 rounded px-1 py-0.5 outline-none text-xs appearance-none cursor-pointer">
          <option :value="5" class="bg-slate-700 text-slate-300">5s</option>
          <option :value="15" class="bg-slate-700 text-slate-300">15s</option>
          <option :value="30" class="bg-slate-700 text-slate-300">30s</option>
          <option :value="60" class="bg-slate-700 text-slate-300">1min</option>
          <option :value="300" class="bg-slate-700 text-slate-300">5min</option>
        </select>
      </div>
    </div>

    <!-- Zoom controls -->
    <div class="absolute top-2 right-2 z-10 flex gap-1">
      <button @click="zoomIn" class="w-7 h-7 bg-slate-700 hover:bg-slate-600 rounded text-slate-300 text-xs"><i class="fa-solid fa-plus"></i></button>
      <button @click="zoomOut" class="w-7 h-7 bg-slate-700 hover:bg-slate-600 rounded text-slate-300 text-xs"><i class="fa-solid fa-minus"></i></button>
      <button @click="zoomFit" class="w-7 h-7 bg-slate-700 hover:bg-slate-600 rounded text-slate-300 text-xs"><i class="fa-solid fa-expand"></i></button>
    </div>

    <div v-if="loading" class="absolute inset-0 flex items-center justify-center bg-slate-800/50 z-20">
      <i class="fa-solid fa-spinner fa-spin text-indigo-400 text-xl"></i>
    </div>

    <!-- Empty state: no selection -->
    <div v-if="!loading && !hasSelection" class="flex flex-col items-center justify-center h-full text-slate-500">
      <i class="fa-solid fa-arrow-pointer text-3xl mb-3 text-slate-600"></i>
      <p class="text-sm">Select an event in the table</p>
      <p class="text-xs mt-1" v-if="mode === 'related'">Timeline will show related events from same session, transaction, and concurrent activity</p>
      <p class="text-xs mt-1" v-else>Timeline will show all filtered events within the time window around the selected event</p>
    </div>

    <!-- Empty state: no results -->
    <div v-if="!loading && timelineEvents.length === 0 && hasSelection" class="flex flex-col items-center justify-center h-full text-slate-500">
      <i class="fa-solid fa-link-slash text-3xl mb-3 text-slate-600"></i>
      <p class="text-sm">No events found within this time window</p>
      <p class="text-xs mt-1">Try increasing the time window{{ mode === 'related' ? ' or switching to "All Filtered" mode' : ' or adjusting filters' }}</p>
    </div>

    <div ref="viewport" class="flex-1 overflow-hidden" @click.self="tooltip = null"></div>

    <!-- Tooltip -->
    <div
      v-if="tooltip"
      class="absolute z-30 bg-slate-800 border border-slate-600 rounded-lg shadow-xl px-3 py-2 text-xs max-w-xs pointer-events-auto"
      :style="{ left: tooltip.x + 'px', top: tooltip.y + 'px' }"
    >
      <button @click="tooltip = null" class="absolute top-1 right-1.5 text-slate-500 hover:text-slate-300">
        <i class="fa-solid fa-xmark"></i>
      </button>
      <div class="font-medium text-slate-200 mb-1">{{ tooltip.event.eventName }}</div>
      <div class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-slate-400">
        <span>Time</span><span class="text-slate-300">{{ formatTimestamp(tooltip.event.timestamp) }}</span>
        <span>Session</span><span class="text-slate-300">{{ tooltip.event.sessionId }}</span>
        <template v-if="tooltip.event.durationUs !== null">
          <span>Duration</span><span class="text-slate-300">{{ formatDuration(tooltip.event.durationUs) }}</span>
        </template>
        <template v-if="tooltip.event.cpuTimeUs !== null">
          <span>CPU</span><span class="text-slate-300">{{ formatDuration(tooltip.event.cpuTimeUs) }}</span>
        </template>
        <template v-if="tooltip.event.durationUs && tooltip.event.cpuTimeUs && tooltip.event.durationUs > tooltip.event.cpuTimeUs * 2">
          <span>Wait ratio</span>
          <span class="text-orange-300 font-medium">
            {{ Math.round((1 - tooltip.event.cpuTimeUs / tooltip.event.durationUs) * 100) }}% waiting
          </span>
        </template>
        <template v-if="tooltip.event.objectName">
          <span>Object</span><span class="text-slate-300 truncate">{{ tooltip.event.objectName }}</span>
        </template>
        <template v-if="tooltip.event.result && tooltip.event.result !== 'OK'">
          <span>Result</span><span class="text-red-400">{{ tooltip.event.result }}</span>
        </template>
        <template v-if="tooltip.event.logicalReads !== null">
          <span>Reads</span><span class="text-slate-300">{{ tooltip.event.logicalReads?.toLocaleString() }}</span>
        </template>
        <template v-if="tooltip.event.waitType">
          <span>Wait</span><span class="text-orange-300">{{ tooltip.event.waitType }}</span>
        </template>
        <template v-if="tooltip.event.lockMode">
          <span>Lock</span><span class="text-yellow-300" :title="getLockModeDescription(tooltip.event.lockMode) ?? ''">{{ tooltip.event.lockMode }}<span v-if="getLockModeDescription(tooltip.event.lockMode)" class="text-yellow-300/50 text-[9px] ml-1">{{ getLockModeDescription(tooltip.event.lockMode)?.split('—')[0]?.trim() }}</span></span>
        </template>
      </div>
      <button
        @click="selectEvent(tooltip.event); tooltip = null"
        class="mt-2 w-full px-2 py-1 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-xs transition-colors"
      >
        <i class="fa-solid fa-crosshairs mr-1"></i>Set as Focus
      </button>
    </div>

    <!-- Info -->
    <div class="shrink-0 px-3 py-1 bg-slate-700/50 border-t border-slate-600 text-xs text-slate-400 flex items-center gap-3">
      <span>{{ timelineEvents.length.toLocaleString() }} events</span>
      <span class="border-l border-slate-600 pl-3 flex items-center gap-2">
        <span><span class="inline-block w-3 h-1 bg-green-500 mr-0.5 rounded-sm align-middle"></span>CPU%</span>
        <span><span class="text-red-400 text-[10px]">&#x2716;</span> Error</span>
        <span><span class="text-red-400 text-[10px]">&#x2620;</span> Deadlock</span>
        <span><span class="text-orange-400 text-[10px]">&#x26D4;</span> Blocked</span>
      </span>
      <span v-if="mode === 'related' && hasSelection" class="border-l border-slate-600 pl-3">
        Related: same session + transaction + resource + concurrent within {{ timeWindow }}s window
      </span>
      <span v-else-if="mode === 'filtered' && hasSelection" class="border-l border-slate-600 pl-3">
        Filtered: active filters within {{ timeWindow }}s window
      </span>
    </div>
  </div>
</template>
