<script setup lang="ts">
import { ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useQueryExecution } from '../composables/useQueryExecution';
import { usePlanState } from '../composables/planState';
import ResultTable from './ResultTable.vue';

const router = useRouter();
const { state, closeResultTab } = useQueryExecution();
const { loadPlan } = usePlanState();

type SubTab = 'messages' | 'results' | 'plan';
const activeSubTab = ref<SubTab>('messages');

// Auto-select the most relevant sub-tab when active result changes
watch(() => state.activeResultTab, () => {
  const tab = state.results[state.activeResultTab];
  if (tab?.result?.planXml) {
    activeSubTab.value = 'plan';
  } else if (tab?.result?.columns.length) {
    activeSubTab.value = 'results';
  } else {
    activeSubTab.value = 'messages';
  }
});

// Also trigger when a new result is pushed
watch(() => state.results.length, () => {
  const tab = state.results[state.activeResultTab];
  if (tab?.result?.planXml) {
    activeSubTab.value = 'plan';
  } else if (tab?.result?.columns.length) {
    activeSubTab.value = 'results';
  } else {
    activeSubTab.value = 'messages';
  }
});

const viewInPlanViewer = (planXml: string) => {
  loadPlan(planXml);
  router.push('/plan-viewer');
};

const formatTime = (date: Date) => {
  return date.toLocaleTimeString();
};
</script>

<template>
  <div class="flex flex-col h-full bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div class="px-4 py-2 bg-slate-700 border-b border-slate-600 flex items-center justify-between">
      <h3 class="flex items-center gap-2 text-sm font-bold text-white">
        <i class="fa-solid fa-terminal text-blue-400"></i>
        Output
      </h3>
    </div>

    <!-- Result Tabs -->
    <div v-if="state.results.length > 0" class="flex items-center bg-slate-800 border-b border-slate-600 px-2 overflow-x-auto">
      <button
        v-for="(tab, idx) in state.results"
        :key="tab.id"
        class="group flex items-center gap-1.5 px-3 py-1.5 text-xs whitespace-nowrap border-b-2 -mb-px transition-colors"
        :class="state.activeResultTab === idx
          ? 'text-indigo-300 border-indigo-400 bg-slate-700/50'
          : 'text-slate-400 border-transparent hover:text-slate-300'"
        @click="state.activeResultTab = idx"
      >
        <i :class="tab.error ? 'fa-solid fa-circle-xmark text-red-400' : 'fa-solid fa-circle-check text-green-400'" class="text-[10px]"></i>
        {{ formatTime(tab.timestamp) }}
        <span
          class="ml-1 text-slate-500 hover:text-red-400 transition-colors"
          @click.stop="closeResultTab(idx)"
        >
          <i class="fa-solid fa-xmark"></i>
        </span>
      </button>
    </div>

    <!-- Content -->
    <div v-if="state.results.length > 0 && state.results[state.activeResultTab]" class="flex-1 flex flex-col overflow-hidden">
      <!-- Sub-tabs -->
      <div class="flex bg-slate-750 border-b border-slate-600 px-4">
        <button
          class="px-3 py-1.5 text-xs font-medium border-b -mb-px transition-colors"
          :class="activeSubTab === 'messages' ? 'text-white border-white' : 'text-slate-500 border-transparent hover:text-slate-300'"
          @click="activeSubTab = 'messages'"
        >
          Messages
        </button>
        <button
          v-if="state.results[state.activeResultTab].result?.columns.length"
          class="px-3 py-1.5 text-xs font-medium border-b -mb-px transition-colors"
          :class="activeSubTab === 'results' ? 'text-white border-white' : 'text-slate-500 border-transparent hover:text-slate-300'"
          @click="activeSubTab = 'results'"
        >
          Results ({{ state.results[state.activeResultTab].result?.rowsAffected || 0 }} rows)
        </button>
        <button
          v-if="state.results[state.activeResultTab].result?.planXml"
          class="px-3 py-1.5 text-xs font-medium border-b -mb-px transition-colors"
          :class="activeSubTab === 'plan' ? 'text-white border-white' : 'text-slate-500 border-transparent hover:text-slate-300'"
          @click="activeSubTab = 'plan'"
        >
          <i class="fa-solid fa-diagram-project mr-1"></i>
          Execution Plan
        </button>
      </div>

      <!-- Messages -->
      <div v-if="activeSubTab === 'messages'" class="flex-1 p-4 overflow-y-auto font-mono text-sm">
        <div v-if="state.results[state.activeResultTab].error" class="text-red-400 mb-2">
          <i class="fa-solid fa-circle-xmark mr-1"></i>
          {{ state.results[state.activeResultTab].error }}
        </div>
        <div
          v-for="(msg, i) in state.results[state.activeResultTab].result?.messages || []"
          :key="i"
          class="text-slate-300 mb-1"
        >
          {{ msg }}
        </div>
        <div class="text-slate-500 mt-2 text-xs">
          Query: {{ state.results[state.activeResultTab].query }}
        </div>
      </div>

      <!-- Results Table -->
      <div v-if="activeSubTab === 'results' && state.results[state.activeResultTab].result" class="flex-1 overflow-hidden">
        <ResultTable
          :columns="state.results[state.activeResultTab].result!.columns"
          :rows="state.results[state.activeResultTab].result!.rows"
        />
      </div>

      <!-- Plan -->
      <div v-if="activeSubTab === 'plan' && state.results[state.activeResultTab].result?.planXml" class="flex-1 p-4 flex flex-col items-center justify-center gap-4">
        <p class="text-slate-400 text-sm">Execution plan captured successfully</p>
        <button
          class="px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-medium flex items-center gap-2 transition-colors"
          @click="viewInPlanViewer(state.results[state.activeResultTab].result!.planXml!)"
        >
          <i class="fa-solid fa-diagram-project"></i>
          View in Plan Viewer
        </button>
        <p class="text-slate-600 text-xs">Plan type: {{ state.results[state.activeResultTab].planType }}</p>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="flex-1 flex items-center justify-center text-slate-600 text-sm">
      <div class="text-center">
        <i class="fa-solid fa-terminal text-3xl mb-2"></i>
        <p>Execute a query to see results</p>
      </div>
    </div>
  </div>
</template>
