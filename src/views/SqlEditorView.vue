<script setup lang="ts">
import SqlEditor from '../components/SqlEditor.vue';
import OutputPanel from '../components/OutputPanel.vue';
</script>

<template>
  <div class="flex-1 grid grid-rows-[1fr_280px] gap-4 p-4 overflow-hidden">
    <!-- SQL Editor -->
    <main class="overflow-hidden">
      <SqlEditor />
    </main>

    <!-- Bottom: Output Panel -->
    <section class="overflow-hidden">
      <OutputPanel />
    </section>
  </div>
</template>
