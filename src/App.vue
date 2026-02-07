<script setup lang="ts">
import { ref } from 'vue';
import ExecutionPlanGraph from './components/ExecutionPlanGraph.vue';
import NodeDetails from './components/NodeDetails.vue';
import PlanLoader from './components/PlanLoader.vue';
import AnalysisPanel from './components/AnalysisPanel.vue';
import PlanComparison from './components/PlanComparison.vue';
import { usePlanState } from './composables/planState';

const { state, loadComparisonPlan, toggleComparisonMode } = usePlanState();

// File input for comparison plan
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
  input.value = ''; // Reset for re-selection
};
</script>

<template>
  <div class="w-screen h-screen flex flex-col bg-slate-900 overflow-hidden">
    <!-- Hidden file input for comparison -->
    <input 
      ref="comparisonFileInput" 
      type="file" 
      accept=".sqlplan,.xml" 
      class="hidden"
      @change="handleComparisonFile"
    />
    
    <!-- Header -->
    <header class="flex items-center justify-between px-6 py-4 bg-gradient-to-r from-indigo-900 to-purple-900 border-b-2 border-indigo-500 shadow-lg">
      <div>
        <h1 class="text-2xl font-black text-white tracking-tight">
          <i class="fa-solid fa-diagram-project mr-2 text-indigo-400"></i>
          SQL Execution Plan Analyzer
        </h1>
        <p class="text-sm text-indigo-200 mt-0.5">Interactive Query Performance Visualization</p>
      </div>
      <div class="flex items-center gap-3">
        <!-- Compare Plans Button -->
        <button
          v-if="state.plan"
          class="px-3 py-1.5 rounded-lg text-sm flex items-center gap-2 transition-colors"
          :class="state.comparisonMode 
            ? 'bg-blue-600 text-white hover:bg-blue-500' 
            : 'bg-slate-800/50 hover:bg-slate-700/50 text-slate-300'"
          @click="state.comparisonPlan ? toggleComparisonMode() : openComparisonFilePicker()"
        >
          <i class="fa-solid fa-code-compare"></i>
          {{ state.comparisonMode ? 'Hide Compare' : 'Compare Plans' }}
        </button>
        <a 
          href="https://github.com/PsyChonek/SqlPlanForDummies" 
          target="_blank"
          class="px-3 py-1.5 bg-slate-800/50 hover:bg-slate-700/50 text-slate-300 rounded-lg text-sm flex items-center gap-2 transition-colors"
        >
          <i class="fa-brands fa-github"></i>
          GitHub
        </a>
      </div>
    </header>
    
    <!-- Main Grid -->
    <div 
      class="flex-1 gap-4 p-4 overflow-hidden"
      :class="state.comparisonMode && state.comparisonPlan
        ? 'grid grid-cols-[320px_1fr_380px] grid-rows-[1fr_280px]'
        : 'grid grid-cols-[320px_1fr_380px] grid-rows-[1fr_280px]'"
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