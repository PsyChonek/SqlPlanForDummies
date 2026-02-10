<script setup lang="ts">
import { ref } from 'vue';
import ExecutionPlanGraph from '../components/ExecutionPlanGraph.vue';
import NodeDetails from '../components/NodeDetails.vue';
import PlanLoader from '../components/PlanLoader.vue';
import AnalysisPanel from '../components/AnalysisPanel.vue';
import PlanComparison from '../components/PlanComparison.vue';
import { usePlanState } from '../composables/planState';

const { state, loadComparisonPlan, toggleComparisonMode } = usePlanState();

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
      class="flex-1 gap-4 p-4 overflow-hidden grid grid-cols-[320px_1fr_380px] grid-rows-[1fr_280px]"
    >
      <!-- Left Sidebar: Plan Loader -->
      <aside class="row-span-2 overflow-hidden">
        <PlanLoader />
      </aside>

      <!-- Main: Execution Plan Graph -->
      <main class="overflow-hidden">
        <ExecutionPlanGraph />
      </main>

      <!-- Right Sidebar: Node Details or Comparison -->
      <aside class="row-span-2 overflow-hidden">
        <PlanComparison v-if="state.comparisonMode && state.comparisonPlan" />
        <NodeDetails v-else />
      </aside>

      <!-- Bottom: Analysis Panel -->
      <section class="overflow-hidden">
        <AnalysisPanel />
      </section>
    </div>
  </div>
</template>
