<script setup lang="ts">
import { useXelState } from '../composables/useXelState';
import type { XelView } from '../composables/useXelState';
import XelFileLoader from '../components/xel/XelFileLoader.vue';
import XelEventTable from '../components/xel/XelEventTable.vue';
import XelEventFilters from '../components/xel/XelEventFilters.vue';
import XelEventDetails from '../components/xel/XelEventDetails.vue';
import XelTimeline from '../components/xel/XelTimeline.vue';
import XelLockChainDiagram from '../components/xel/XelLockChainDiagram.vue';
import XelSummaryDashboard from '../components/xel/XelSummaryDashboard.vue';

const { state, setActiveView, hasData } = useXelState();

const tabs: { id: XelView; label: string; icon: string }[] = [
  { id: 'table', label: 'Events', icon: 'fa-table' },
  { id: 'timeline', label: 'Timeline', icon: 'fa-chart-gantt' },
  { id: 'lockchain', label: 'Lock Chain', icon: 'fa-diagram-project' },
  { id: 'dashboard', label: 'Dashboard', icon: 'fa-chart-pie' },
];
</script>

<template>
  <div class="flex-1 gap-4 p-4 overflow-hidden grid grid-cols-[280px_1fr_360px] grid-rows-[1fr]">
    <!-- Left: File Loader -->
    <aside class="overflow-hidden flex flex-col">
      <XelFileLoader />
    </aside>

    <!-- Center: Tabbed Content -->
    <main class="overflow-hidden flex flex-col bg-slate-800 rounded-2xl shadow-xl">
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

    <!-- Right: Event Details -->
    <aside class="overflow-hidden flex flex-col">
      <XelEventDetails />
    </aside>
  </div>
</template>
