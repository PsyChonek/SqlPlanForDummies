<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import type { XelEvent } from '../../types/xel';
import { getEventSeverity, getEventSeverityBg, formatDuration, formatNumber, formatTimestamp, getLockModeDescription } from '../../types/xel';

const { state, selectEvent } = useXelState();

const ROW_HEIGHT = 28;
const FETCH_SIZE = 500;

const containerRef = ref<HTMLDivElement | null>(null);
const containerHeight = ref(600);
const scrollTop = ref(0);
const totalCount = ref(0);
const loading = ref(false);

const cache = ref<Map<number, XelEvent[]>>(new Map());
const allColumns = ref<string[]>([]);

// Sort state
const sortBy = ref<string | null>('timestamp');
const sortDesc = ref(false);

// Column display config
const COLUMN_WIDTHS: Record<string, string> = {
  id: '50px',
  timestamp: '170px',
  eventName: '150px',
  sessionId: '100px',
  durationUs: '130px',
  cpuTimeUs: '130px',
  logicalReads: '130px',
  physicalReads: '130px',
  writes: '70px',
  result: '70px',
  objectName: '160px',
  statement: '300px',
  sqlText: '300px',
  resourceType: '130px',
  lockMode: '100px',
  resourceDescription: '160px',
  waitType: '110px',
  waitDurationMs: '140px',
  username: '110px',
  clientAppName: '130px',
  databaseName: '120px',
  sourceFile: '140px',
};


const SHORT_LABELS: Record<string, string> = {
  sessionId: 'Session ID',
  durationUs: 'Duration (µs)',
  cpuTimeUs: 'CPU Time (µs)',
  logicalReads: 'Logical Reads',
  physicalReads: 'Physical Reads',
  waitDurationMs: 'Wait Duration',
  clientAppName: 'Client App',
  databaseName: 'Database',
  resourceType: 'Resource Type',
  resourceDescription: 'Resource Desc',
  objectName: 'Object Name',
  eventName: 'Event Name',
  lockMode: 'Lock Mode',
  waitType: 'Wait Type',
  sourceFile: 'Source File',
};

// Columns to show by default (hide verbose ones initially)
const HIDDEN_BY_DEFAULT = new Set([
  'id', 'sqlText', 'statement', 'sourceFile',
  'blockedProcessReport', 'deadlockGraph', 'physicalReads', 'writes',
  'resourceDescription',
]);

const visibleColumns = ref<Set<string>>(new Set());
const showColumnPicker = ref(false);

const initColumns = (cols: string[]) => {
  allColumns.value = cols;
  if (visibleColumns.value.size === 0) {
    visibleColumns.value = new Set(cols.filter(c => !HIDDEN_BY_DEFAULT.has(c)));
  } else {
    // Add any new columns from newly loaded file (keep existing visibility)
    for (const c of cols) {
      // new column not seen before — make visible
      if (!allColumns.value.includes(c)) {
        visibleColumns.value.add(c);
      }
    }
  }
};

const displayColumns = computed(() =>
  allColumns.value.filter(c => visibleColumns.value.has(c))
);

const toggleColumn = (col: string) => {
  if (visibleColumns.value.has(col)) {
    visibleColumns.value.delete(col);
  } else {
    visibleColumns.value.add(col);
  }
  visibleColumns.value = new Set(visibleColumns.value); // trigger reactivity
};

// Runtime column widths (px numbers) — overridden by drag resize
const columnWidthsPx = ref<Record<string, number>>({});

const getDefaultWidthPx = (col: string): number => {
  if (COLUMN_WIDTHS[col]) return parseInt(COLUMN_WIDTHS[col], 10);
  const label = getColLabel(col);
  return Math.max(100, label.length * 8 + 36);
};

const getColWidthPx = (col: string): number =>
  columnWidthsPx.value[col] ?? getDefaultWidthPx(col);

const getColWidth = (col: string) => `${getColWidthPx(col)}px`;
const getColLabel = (col: string) => SHORT_LABELS[col] || col;

// Column resize drag
const resizing = ref<{ col: string; startX: number; startWidth: number } | null>(null);
let justResized = false;

const onResizeStart = (col: string, e: MouseEvent) => {
  e.stopPropagation();
  e.preventDefault();
  resizing.value = { col, startX: e.clientX, startWidth: getColWidthPx(col) };
  document.addEventListener('mousemove', onResizeMove);
  document.addEventListener('mouseup', onResizeEnd);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

const onResizeMove = (e: MouseEvent) => {
  if (!resizing.value) return;
  const diff = e.clientX - resizing.value.startX;
  const newWidth = Math.max(40, resizing.value.startWidth + diff);
  columnWidthsPx.value = { ...columnWidthsPx.value, [resizing.value.col]: newWidth };
};

const onResizeEnd = () => {
  resizing.value = null;
  justResized = true;
  requestAnimationFrame(() => { justResized = false; });
  document.removeEventListener('mousemove', onResizeMove);
  document.removeEventListener('mouseup', onResizeEnd);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};

// Total row width for full-width backgrounds
const totalRowWidth = computed(() =>
  displayColumns.value.reduce((sum, col) => sum + getColWidthPx(col), 0)
);

// Virtual scroll
const totalHeight = computed(() => totalCount.value * ROW_HEIGHT);
const visibleStart = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - 5));
const visibleEnd = computed(() => {
  const rowsVisible = Math.ceil(containerHeight.value / ROW_HEIGHT);
  return Math.min(totalCount.value, Math.floor(scrollTop.value / ROW_HEIGHT) + rowsVisible + 5);
});

const getEventAt = (globalIdx: number): XelEvent | null => {
  const page = Math.floor(globalIdx / FETCH_SIZE);
  const pageEvents = cache.value.get(page);
  if (!pageEvents) return null;
  return pageEvents[globalIdx - page * FETCH_SIZE] ?? null;
};

const visibleRows = computed(() => {
  const rows: { event: XelEvent; globalIdx: number }[] = [];
  for (let i = visibleStart.value; i < visibleEnd.value; i++) {
    const event = getEventAt(i);
    if (event) rows.push({ event, globalIdx: i });
  }
  return rows;
});

const offsetStyle = computed(() => ({
  transform: `translateY(${visibleStart.value * ROW_HEIGHT}px)`,
}));

// Cell value extraction — works for both fixed and extra fields
const getCellValue = (event: XelEvent, key: string): string => {
  // Check fixed fields first
  const fixedVal = getFixedField(event, key);
  if (fixedVal !== undefined) return formatValue(key, fixedVal);

  // Check extra_fields
  const extra = event.extraFields?.[key];
  if (extra !== undefined && extra !== null) return String(extra);

  return '';
};

const getFixedField = (event: XelEvent, key: string): unknown | undefined => {
  switch (key) {
    case 'id': return event.id;
    case 'timestamp': return event.timestamp;
    case 'eventName': return event.eventName;
    case 'sessionId': return event.sessionId;
    case 'durationUs': return event.durationUs;
    case 'cpuTimeUs': return event.cpuTimeUs;
    case 'logicalReads': return event.logicalReads;
    case 'physicalReads': return event.physicalReads;
    case 'writes': return event.writes;
    case 'result': return event.result;
    case 'objectName': return event.objectName;
    case 'statement': return event.statement;
    case 'sqlText': return event.sqlText;
    case 'resourceType': return event.resourceType;
    case 'lockMode': return event.lockMode;
    case 'resourceDescription': return event.resourceDescription;
    case 'waitType': return event.waitType;
    case 'waitDurationMs': return event.waitDurationMs;
    case 'username': return event.username;
    case 'clientAppName': return event.clientAppName;
    case 'databaseName': return event.databaseName;
    case 'sourceFile': return event.sourceFile;
    default: return undefined;
  }
};

const formatValue = (key: string, val: unknown): string => {
  if (val === null || val === undefined) return '';
  if (key === 'timestamp') return formatTimestamp(val as string);
  if (key === 'durationUs' || key === 'cpuTimeUs') return formatDuration(val as number | null);
  if (key === 'logicalReads' || key === 'physicalReads' || key === 'writes')
    return formatNumber(val as number | null);
  if (key === 'sourceFile') return (val as string).split(/[/\\]/).pop() || (val as string);
  return String(val);
};

// Data fetching
const fetchPage = async (page: number) => {
  if (cache.value.has(page) || loading.value) return;
  loading.value = true;
  try {
    const response = await xelApi.queryEvents({
      filter: { ...state.filter },
      offset: page * FETCH_SIZE,
      limit: FETCH_SIZE,
      sortBy: sortBy.value,
      sortDesc: sortDesc.value,
    });
    cache.value.set(page, response.events);
    totalCount.value = response.totalCount;
  } catch (err) {
    console.error('Failed to fetch events:', err);
  } finally {
    loading.value = false;
  }
};

const ensureVisible = async () => {
  const startPage = Math.floor(visibleStart.value / FETCH_SIZE);
  const endPage = Math.floor(Math.max(0, visibleEnd.value - 1) / FETCH_SIZE);
  for (let p = startPage; p <= endPage; p++) {
    await fetchPage(p);
  }
};

const onScroll = () => {
  if (!containerRef.value) return;
  scrollTop.value = containerRef.value.scrollTop;
  ensureVisible();
};

const resetAndFetch = async () => {
  cache.value = new Map();
  totalCount.value = 0;
  if (containerRef.value) containerRef.value.scrollTop = 0;
  scrollTop.value = 0;
  // Refresh columns
  try {
    const cols = await xelApi.getColumns();
    initColumns(cols);
  } catch {}
  await fetchPage(0);
};

const toggleSort = (key: string) => {
  if (justResized) return;
  if (sortBy.value === key) {
    sortDesc.value = !sortDesc.value;
    if (!sortDesc.value && sortBy.value === key) {
      // Second click was desc, third click clears
    }
  } else {
    sortBy.value = key;
    sortDesc.value = false;
  }
  cache.value = new Map();
  if (containerRef.value) containerRef.value.scrollTop = 0;
  scrollTop.value = 0;
  fetchPage(0);
};

watch(() => state.revision, resetAndFetch);

let resizeObserver: ResizeObserver | null = null;

onMounted(async () => {
  if (containerRef.value) {
    containerHeight.value = containerRef.value.clientHeight;
    resizeObserver = new ResizeObserver(entries => {
      containerHeight.value = entries[0].contentRect.height;
    });
    resizeObserver.observe(containerRef.value);
  }
  try {
    const cols = await xelApi.getColumns();
    initColumns(cols);
  } catch {}
  await fetchPage(0);
});

onUnmounted(() => {
  resizeObserver?.disconnect();
});
</script>

<template>
  <div class="flex flex-col h-full relative">
    <!-- Toolbar -->
    <div class="shrink-0 flex items-center justify-between px-2 py-1 bg-slate-700/30 border-b border-slate-600">
      <span class="text-xs text-slate-500">{{ displayColumns.length }}/{{ allColumns.length }} columns</span>
      <button
        @click="showColumnPicker = !showColumnPicker"
        class="text-xs px-2 py-0.5 rounded bg-slate-700 text-slate-400 hover:text-slate-200 transition-colors"
      >
        <i class="fa-solid fa-table-columns mr-1"></i>Columns
      </button>
    </div>

    <!-- Column picker dropdown -->
    <div v-if="showColumnPicker" class="shrink-0 max-h-48 overflow-auto px-2 py-2 bg-slate-750 border-b border-slate-600 flex flex-wrap gap-1">
      <button
        v-for="col in allColumns"
        :key="col"
        @click="toggleColumn(col)"
        class="text-xs px-2 py-0.5 rounded border transition-colors"
        :class="visibleColumns.has(col)
          ? 'bg-indigo-900/40 border-indigo-600/50 text-indigo-300'
          : 'bg-slate-800 border-slate-600 text-slate-500 hover:text-slate-300'"
      >
        {{ getColLabel(col) }}
      </button>
    </div>

    <!-- Virtual scroll container -->
    <div
      ref="containerRef"
      class="flex-1 overflow-auto"
      @scroll="onScroll"
    >
      <!-- Sticky Header -->
      <div class="flex sticky top-0 z-10 bg-slate-800 border-b border-slate-600 text-xs text-slate-400 font-medium uppercase tracking-wider" :style="{ minWidth: `${totalRowWidth}px` }">
        <div
          v-for="col in displayColumns"
          :key="col"
          :style="{ width: getColWidth(col), minWidth: getColWidth(col), flexShrink: 0 }"
          class="relative px-2.5 py-1.5 cursor-pointer hover:text-slate-200 transition-colors select-none flex items-center gap-0.5 whitespace-nowrap border-r border-slate-600/50"
          @click="toggleSort(col)"
          :title="col"
        >
          <span class="whitespace-nowrap">{{ getColLabel(col) }}</span>
          <i
            v-if="sortBy === col"
            :class="['fa-solid text-indigo-400', sortDesc ? 'fa-sort-down' : 'fa-sort-up']"
          ></i>
          <!-- Resize handle -->
          <div
            class="xel-resize-handle"
            @mousedown="onResizeStart(col, $event)"
          ></div>
        </div>
      </div>

      <div :style="{ height: `${totalHeight}px`, position: 'relative' }">
        <div :style="offsetStyle">
          <div
            v-for="row in visibleRows"
            :key="row.event.id"
            :style="{ height: `${ROW_HEIGHT}px`, minWidth: `${totalRowWidth}px` }"
            class="group flex items-center border-b cursor-pointer transition-colors text-xs"
            :class="[
              state.selectedEvent?.id === row.event.id
                ? 'xel-row-selected'
                : getEventSeverityBg(getEventSeverity(row.event)) || 'border-slate-700/30',
            ]"
            @click="selectEvent(row.event)"
          >
            <div
              v-for="col in displayColumns"
              :key="col"
              :style="{ width: getColWidth(col), minWidth: getColWidth(col), flexShrink: 0 }"
              class="px-2.5 truncate border-r border-slate-700/50"
              :class="col === 'result' && row.event.result === 'Error' ? 'text-red-400' : 'text-slate-300'"
              :title="col === 'lockMode' && row.event.lockMode ? row.event.lockMode + ' — ' + (getLockModeDescription(row.event.lockMode) || '') : getCellValue(row.event, col)"
            >
              {{ getCellValue(row.event, col) }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Loading overlay -->
    <div v-if="loading" class="absolute inset-0 z-20 flex items-center justify-center bg-slate-800/70 pointer-events-none">
      <div class="flex flex-col items-center gap-2">
        <i class="fa-solid fa-spinner fa-spin text-2xl text-indigo-400"></i>
        <span class="text-xs text-slate-300">Loading events...</span>
      </div>
    </div>

    <!-- Footer -->
    <div class="shrink-0 flex items-center justify-between px-3 py-1 bg-slate-700/50 border-t border-slate-600 text-xs text-slate-400">
      <span>{{ totalCount.toLocaleString() }} events</span>
      <span v-if="loading">
        <i class="fa-solid fa-spinner fa-spin mr-1"></i>Loading...
      </span>
    </div>
  </div>
</template>

<style scoped>
.xel-row-selected {
  background-color: rgb(49 46 129 / 0.5) !important;
  border-color: rgb(79 70 229 / 0.5) !important;
}

.group:not(.xel-row-selected):hover {
  background-color: rgb(51 65 85 / 0.4) !important;
}

.xel-resize-handle {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 1;
}

.xel-resize-handle:hover,
.xel-resize-handle:active {
  background-color: rgb(99 102 241 / 0.5);
}
</style>
