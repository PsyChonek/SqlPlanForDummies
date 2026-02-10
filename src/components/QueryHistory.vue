<script setup lang="ts">
import { onMounted } from 'vue';
import { useQueryHistory } from '../composables/useQueryHistory';

const { state, loadHistory, filteredQueries, getPlansForQuery } = useQueryHistory();

const emit = defineEmits<{
  loadQuery: [sql: string];
  viewPlan: [planXml: string];
}>();

const formatDate = (dateStr: string) => {
  const d = new Date(dateStr);
  return d.toLocaleString();
};

const formatDuration = (ms: number) => {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
};

onMounted(() => {
  loadHistory();
});
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- Search -->
    <div class="px-3 py-2 border-b border-slate-600">
      <input
        v-model="state.searchTerm"
        type="text"
        placeholder="Search queries..."
        class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
      />
    </div>

    <!-- Query List -->
    <div class="flex-1 overflow-y-auto p-2 space-y-2">
      <div v-if="filteredQueries.length === 0" class="text-center text-slate-600 text-sm py-4">
        No queries yet
      </div>

      <div
        v-for="entry in filteredQueries"
        :key="entry.id"
        class="p-2 rounded-lg bg-slate-700/50 border border-slate-600 hover:border-slate-500 transition-colors"
      >
        <div class="flex items-start justify-between gap-2">
          <p
            class="text-xs font-mono text-slate-300 truncate flex-1 cursor-pointer hover:text-white"
            @click="emit('loadQuery', entry.sql)"
            :title="entry.sql"
          >
            {{ entry.sql.substring(0, 120) }}
          </p>
          <span
            :class="entry.success ? 'text-green-500' : 'text-red-500'"
            class="text-[10px] flex-shrink-0"
          >
            <i :class="entry.success ? 'fa-solid fa-check' : 'fa-solid fa-xmark'"></i>
          </span>
        </div>
        <div class="flex items-center gap-3 mt-1 text-[10px] text-slate-500">
          <span>{{ formatDate(entry.executedAt) }}</span>
          <span>{{ formatDuration(entry.durationMs) }}</span>
          <span>{{ entry.connectionName }}</span>
        </div>

        <!-- Plans for this query -->
        <div v-if="getPlansForQuery(entry.sql.substring(0, 100)).length > 0" class="mt-1.5">
          <div class="flex items-center gap-1 flex-wrap">
            <button
              v-for="plan in getPlansForQuery(entry.sql.substring(0, 100))"
              :key="plan.id"
              class="text-[10px] px-1.5 py-0.5 rounded bg-indigo-900/50 text-indigo-300 hover:bg-indigo-800/50 transition-colors"
              @click="emit('viewPlan', plan.planXml)"
            >
              <i class="fa-solid fa-diagram-project mr-0.5"></i>
              {{ plan.planType }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
