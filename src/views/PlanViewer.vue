<script setup lang="ts">
import { ref } from 'vue';
import ExecutionPlanGraph from '../components/ExecutionPlanGraph.vue';
import NodeDetails from '../components/NodeDetails.vue';
import PlanLoader from '../components/PlanLoader.vue';
import AnalysisPanel from '../components/AnalysisPanel.vue';
import PlanComparison from '../components/PlanComparison.vue';
import { usePlanState } from '../composables/planState';

const { state, loadComparisonPlan, toggleComparisonMode } = usePlanState();

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
    <!-- Comparison controls -->
    <div class="flex items-center justify-end px-4 py-2 bg-slate-800/50 border-b border-slate-700">
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
          : 'bg-slate-700 hover:bg-slate-600 text-slate-300'"
        @click="state.comparisonPlan ? toggleComparisonMode() : openComparisonFilePicker()"
      >
        <i class="fa-solid fa-code-compare"></i>
        {{ state.comparisonMode ? 'Hide Compare' : 'Compare Plans' }}
      </button>
    </div>

    <!-- Main Grid -->
    <div
      class="flex-1 gap-4 p-4 overflow-hidden grid grid-cols-[320px_1fr_380px] grid-rows-[1fr]"
    >
      <!-- Left Sidebar: Plan Loader -->
      <aside class="overflow-hidden">
        <PlanLoader />
      </aside>

      <!-- Main: Tabbed panel (Execution Plan / Plan Analysis) -->
      <main class="overflow-hidden flex flex-col bg-slate-800 rounded-2xl shadow-xl">
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
        </div>

        <!-- Tab content -->
        <div class="flex-1 overflow-hidden relative">
          <div v-show="activeMainTab === 'execution'" class="absolute inset-0">
            <ExecutionPlanGraph :show-header="false" />
          </div>
          <div v-show="activeMainTab === 'analysis'" class="absolute inset-0">
            <AnalysisPanel ref="analysisPanelRef" :show-header="false" />
          </div>
        </div>
      </main>

      <!-- Right Sidebar: Node Details or Comparison -->
      <aside class="overflow-hidden">
        <PlanComparison v-if="state.comparisonMode && state.comparisonPlan" />
        <NodeDetails v-else />
      </aside>
    </div>
  </div>
</template>
