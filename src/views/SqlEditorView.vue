<script setup lang="ts">
import SqlEditor from '../components/SqlEditor.vue';
import OutputPanel from '../components/OutputPanel.vue';
import { useResizePanel } from '../composables/useResizePanel';

const bottom = useResizePanel({ initial: 280, direction: 'bottom' });
</script>

<template>
  <div class="flex-1 flex flex-col p-4 gap-0 overflow-hidden">
    <!-- SQL Editor -->
    <main class="overflow-hidden flex-1 min-h-0">
      <SqlEditor />
    </main>

    <!-- Bottom Handle -->
    <div
      class="shrink-0 h-4 flex justify-center items-center cursor-row-resize z-10"
      @pointerdown="bottom.onPointerDown"
      @dblclick="bottom.onDoubleClick"
    >
      <div class="h-0.5 w-8 rounded-full bg-slate-600"></div>
    </div>

    <!-- Bottom: Output Panel -->
    <section
      v-show="!bottom.collapsed.value"
      class="overflow-hidden shrink-0"
      :style="{ height: bottom.size.value + 'px' }"
    >
      <OutputPanel />
    </section>
  </div>
</template>
