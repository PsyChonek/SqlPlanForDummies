<script setup lang="ts">
import { ref } from 'vue';
import ExecutionPlanGraph from '../components/ExecutionPlanGraph.vue';
import NodeDetails from '../components/NodeDetails.vue';
import PlanLoader from '../components/PlanLoader.vue';
import AnalysisPanel from '../components/AnalysisPanel.vue';
import PlanComparison from '../components/PlanComparison.vue';
import { usePlanState } from '../composables/planState';
import { useResizePanel } from '../composables/useResizePanel';

const { state, loadComparisonPlan, toggleComparisonMode } = usePlanState();

const containerRef = ref<HTMLElement | null>(null);
const HANDLE_WIDTH = 16;

const left = useResizePanel({
  initial: 320,
  direction: 'left',
  getMaxSize: () => {
    const total = containerRef.value?.clientWidth ?? 1200;
    const rightSize = right.collapsed.value ? 0 : right.size.value;
    return total - rightSize - HANDLE_WIDTH * 2;
  },
});
const right = useResizePanel({
  initial: 380,
  direction: 'right',
  getMaxSize: () => {
    const total = containerRef.value?.clientWidth ?? 1200;
    const leftSize = left.collapsed.value ? 0 : left.size.value;
    return total - leftSize - HANDLE_WIDTH * 2;
  },
});

type MainTab = 'execution' | 'analysis';
const activeMainTab = ref<MainTab>('execution');
const analysisPanelRef = ref<InstanceType<typeof AnalysisPanel> | null>(null);

const comparisonFileInput = ref<HTMLInputElement | null>(null);

const openComparisonFilePicker = () => {
  comparisonFileInput.value?.click();
};

const handleComparisonFile = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  const text = await file.text();
  loadComparisonPlan(text);
  input.value = '';
};
</script>

<template>
  <div class="flex flex-col h-full overflow-hidden">
    <!-- Main layout -->
    <div ref="containerRef" class="flex-1 flex p-4 gap-0 overflow-hidden">
      <!-- Left Sidebar: Plan Loader -->
      <aside
        v-show="!left.collapsed.value"
        class="overflow-hidden shrink-0"
        :style="{ width: left.size.value + 'px' }"
      >
        <PlanLoader />
      </aside>

      <!-- Left Handle -->
      <div
        class="shrink-0 w-4 flex items-center justify-center cursor-col-resize z-10"
        @pointerdown="left.onPointerDown"
        @dblclick="left.onDoubleClick"
      >
        <div class="w-0.5 h-8 rounded-full bg-slate-600"></div>
      </div>

      <!-- Main: Tabbed panel -->
      <main class="overflow-hidden flex flex-col bg-slate-800 rounded-2xl shadow-xl flex-1 min-w-0">
        <!-- Tab bar -->
        <div class="flex items-center bg-slate-700 border-b border-slate-600 rounded-t-2xl shrink-0">
          <button
            class="flex items-center gap-2 px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'execution'
              ? 'border-blue-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'execution'"
          >
            <i class="fa-solid fa-diagram-project text-blue-400"></i>
            Execution Plan
            <span v-if="activeMainTab === 'execution' && state.selectedStatement" class="text-xs text-slate-400 font-normal ml-1">
              {{ state.selectedStatement.statementSubTreeCost.toFixed(6) }}
            </span>
          </button>
          <button
            class="flex items-center gap-2 px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'analysis'
              ? 'border-purple-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'analysis'"
          >
            <i class="fa-solid fa-microscope text-purple-400"></i>
            Plan Analysis
            <span
              v-if="analysisPanelRef?.issueCount"
              class="px-1.5 py-0.5 bg-amber-500/20 text-amber-400 text-xs font-semibold rounded-full ml-1"
            >
              {{ analysisPanelRef.issueCount }}
            </span>
          </button>
          <div class="ml-auto px-3">
            <input
              ref="comparisonFileInput"
              type="file"
              accept=".sqlplan,.xml"
              class="hidden"
              @change="handleComparisonFile"
            />
            <button
              v-if="state.plan"
              class="px-3 py-1.5 rounded-lg text-sm flex items-center gap-2 transition-colors"
              :class="state.comparisonMode
                ? 'bg-blue-600 text-white hover:bg-blue-500'
                : 'bg-slate-600 hover:bg-slate-500 text-slate-300'"
              @click="state.comparisonPlan ? toggleComparisonMode() : openComparisonFilePicker()"
            >
              <i class="fa-solid fa-code-compare"></i>
              {{ state.comparisonMode ? 'Hide Compare' : 'Compare Plans' }}
            </button>
          </div>
        </div>

        <!-- Tab content -->
        <div class="flex-1 overflow-hidden relative">
          <!-- Loading overlay -->
          <div v-if="state.loading" class="absolute inset-0 z-10 flex items-center justify-center bg-slate-800/80">
            <div class="flex flex-col items-center gap-3">
              <i class="fa-solid fa-spinner fa-spin text-3xl text-blue-400"></i>
              <span class="text-sm text-slate-300">Loading execution plan...</span>
            </div>
          </div>

          <div v-show="activeMainTab === 'execution'" class="absolute inset-0">
            <ExecutionPlanGraph :show-header="false" />
          </div>
          <div v-show="activeMainTab === 'analysis'" class="absolute inset-0">
            <AnalysisPanel ref="analysisPanelRef" :show-header="false" />
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

      <!-- Right Sidebar: Node Details or Comparison -->
      <aside
        v-show="!right.collapsed.value"
        class="overflow-hidden shrink-0"
        :style="{ width: right.size.value + 'px' }"
      >
        <PlanComparison v-if="state.comparisonMode && state.comparisonPlan" />
        <NodeDetails v-else />
      </aside>
    </div>
  </div>
</template>
