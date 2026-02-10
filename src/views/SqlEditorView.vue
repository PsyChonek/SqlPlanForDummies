<script setup lang="ts">
import SqlEditor from '../components/SqlEditor.vue';
import ConnectionManager from '../components/ConnectionManager.vue';
import OutputPanel from '../components/OutputPanel.vue';
</script>

<template>
  <div class="flex-1 grid grid-cols-[1fr_300px] grid-rows-[1fr_280px] gap-4 p-4 overflow-hidden">
    <!-- Center: SQL Editor -->
    <main class="overflow-hidden">
      <SqlEditor />
    </main>

    <!-- Right Sidebar: Connection Manager -->
    <aside class="row-span-2 overflow-hidden">
      <ConnectionManager />
    </aside>

    <!-- Bottom: Output Panel -->
    <section class="overflow-hidden">
      <OutputPanel />
    </section>
  </div>
</template>
