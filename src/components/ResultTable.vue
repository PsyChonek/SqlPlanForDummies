<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  columns: string[];
  rows: any[][];
}>();

const MAX_ROWS = 10000;

const displayRows = computed(() => {
  return props.rows.slice(0, MAX_ROWS);
});

const isTruncated = computed(() => {
  return props.rows.length > MAX_ROWS;
});

const formatCell = (value: any): string => {
  if (value === null || value === undefined) return 'NULL';
  if (typeof value === 'object') return JSON.stringify(value);
  const str = String(value);
  return str.length > 200 ? str.substring(0, 200) + '...' : str;
};
</script>

<template>
  <div class="h-full overflow-auto">
    <table class="w-full text-sm border-collapse">
      <thead class="sticky top-0 z-10">
        <tr>
          <th
            v-for="col in columns"
            :key="col"
            class="px-3 py-2 text-left text-xs font-semibold text-slate-300 bg-slate-700 border-b border-slate-600 whitespace-nowrap"
          >
            {{ col }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="(row, rowIdx) in displayRows"
          :key="rowIdx"
          class="hover:bg-slate-700/50 transition-colors"
          :class="rowIdx % 2 === 0 ? 'bg-slate-800' : 'bg-slate-800/50'"
        >
          <td
            v-for="(cell, colIdx) in row"
            :key="colIdx"
            class="px-3 py-1.5 text-slate-300 border-b border-slate-700/50 whitespace-nowrap font-mono text-xs"
            :class="cell === null || cell === undefined ? 'text-slate-600 italic' : ''"
          >
            {{ formatCell(cell) }}
          </td>
        </tr>
      </tbody>
    </table>

    <div v-if="isTruncated" class="px-4 py-2 text-xs text-amber-400 bg-amber-900/20 border-t border-amber-700/50">
      <i class="fa-solid fa-triangle-exclamation mr-1"></i>
      Results truncated. Showing {{ MAX_ROWS.toLocaleString() }} of {{ rows.length.toLocaleString() }} rows.
    </div>

    <div v-if="rows.length === 0" class="px-4 py-8 text-center text-slate-600 text-sm">
      No data returned
    </div>
  </div>
</template>
