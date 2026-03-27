<script setup lang="ts">
import { ref } from 'vue';
import { useXelState } from '../composables/useXelState';
import { useResizePanel } from '../composables/useResizePanel';
import type { XelView } from '../composables/useXelState';
import XelFileLoader from '../components/xel/XelFileLoader.vue';
import XelEventTable from '../components/xel/XelEventTable.vue';
import XelEventFilters from '../components/xel/XelEventFilters.vue';
import XelEventDetails from '../components/xel/XelEventDetails.vue';
import XelTimeline from '../components/xel/XelTimeline.vue';
import XelLockChainDiagram from '../components/xel/XelLockChainDiagram.vue';
import XelSummaryDashboard from '../components/xel/XelSummaryDashboard.vue';

const { state, setActiveView, hasData } = useXelState();

const containerRef = ref<HTMLElement | null>(null);
const HANDLE_WIDTH = 16;

const left = useResizePanel({
  initial: 280,
  direction: 'left',
  getMaxSize: () => {
    const total = containerRef.value?.clientWidth ?? 1200;
    const rightSize = right.collapsed.value ? 0 : right.size.value;
    return total - rightSize - HANDLE_WIDTH * 2;
  },
});
const right = useResizePanel({
  initial: 360,
  direction: 'right',
  getMaxSize: () => {
    const total = containerRef.value?.clientWidth ?? 1200;
    const leftSize = left.collapsed.value ? 0 : left.size.value;
    return total - leftSize - HANDLE_WIDTH * 2;
  },
});

const tabs: { id: XelView; label: string; icon: string }[] = [
  { id: 'table', label: 'Events', icon: 'fa-table' },
  { id: 'timeline', label: 'Timeline', icon: 'fa-chart-gantt' },
  { id: 'lockchain', label: 'Lock Chain', icon: 'fa-diagram-project' },
  { id: 'dashboard', label: 'Dashboard', icon: 'fa-chart-pie' },
];
</script>

<template>
  <div ref="containerRef" class="flex-1 flex p-4 gap-0 overflow-hidden">
    <!-- Left: File Loader -->
    <aside
      v-show="!left.collapsed.value"
      class="overflow-hidden flex flex-col shrink-0"
      :style="{ width: left.size.value + 'px' }"
    >
      <XelFileLoader />
    </aside>

    <!-- Left Handle -->
    <div
      class="shrink-0 w-4 flex items-center justify-center cursor-col-resize z-10"
      @pointerdown="left.onPointerDown"
      @dblclick="left.onDoubleClick"
    >
      <div class="w-0.5 h-8 rounded-full bg-slate-600"></div>
    </div>

    <!-- Center: Tabbed Content -->
    <main class="overflow-hidden flex flex-col bg-slate-800 rounded-2xl shadow-xl flex-1 min-w-0">
      <!-- Tab bar -->
      <div class="flex items-center bg-slate-700 border-b border-slate-600 rounded-t-2xl shrink-0">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          @click="setActiveView(tab.id)"
          class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
          :class="state.activeView === tab.id
            ? 'text-indigo-300 border-indigo-400'
            : 'text-slate-400 border-transparent hover:text-slate-300 hover:border-slate-600'"
        >
          <i :class="['fa-solid', tab.icon, 'mr-1.5']"></i>
          {{ tab.label }}
        </button>
      </div>

      <!-- Filter bar -->
      <XelEventFilters v-if="hasData" />

      <!-- Content -->
      <div class="flex-1 overflow-hidden relative">
        <!-- Loading overlay (file loading only) -->
        <div v-if="state.loading" class="absolute inset-0 z-10 flex items-center justify-center bg-slate-800/80">
          <div class="flex flex-col items-center gap-3">
            <i class="fa-solid fa-spinner fa-spin text-3xl text-indigo-400"></i>
            <span class="text-sm text-slate-300">Loading XEL data...</span>
          </div>
        </div>

        <template v-if="hasData">
          <div v-show="state.activeView === 'table'" class="absolute inset-0">
            <XelEventTable />
          </div>
          <div v-show="state.activeView === 'timeline'" class="absolute inset-0">
            <XelTimeline />
          </div>
          <div v-show="state.activeView === 'lockchain'" class="absolute inset-0">
            <XelLockChainDiagram />
          </div>
          <div v-show="state.activeView === 'dashboard'" class="absolute inset-0">
            <XelSummaryDashboard />
          </div>
        </template>

        <!-- Empty state -->
        <div v-else-if="!state.loading" class="flex flex-col items-center justify-center h-full text-slate-500">
          <i class="fa-solid fa-chart-gantt text-6xl mb-4 text-slate-600"></i>
          <p class="text-lg font-medium">No XEL data loaded</p>
          <p class="text-sm mt-1">Drop .xel or .xml files in the left panel to begin</p>
        </div>
      </div>
    </main>

    <!-- Right Handle -->
    <div
      class="shrink-0 w-4 flex items-center justify-center cursor-col-resize z-10"
      @pointerdown="right.onPointerDown"
      @dblclick="right.onDoubleClick"
    >
      <div class="w-0.5 h-8 rounded-full bg-slate-600"></div>
    </div>

    <!-- Right: Event Details -->
    <aside
      v-show="!right.collapsed.value"
      class="overflow-hidden flex flex-col shrink-0"
      :style="{ width: right.size.value + 'px' }"
    >
      <XelEventDetails />
    </aside>
  </div>
</template>
