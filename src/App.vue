<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useDbConnection } from './composables/useDbConnection';
import { tauriInvoke } from './composables/tauriApi';
import ConnectionManager from './components/ConnectionManager.vue';

const { state: dbState } = useDbConnection();
const showConnectionDialog = ref(false);
const isWindows = ref(false);

onMounted(async () => {
  try {
    const platform = await tauriInvoke<string>('get_platform');
    isWindows.value = platform === 'windows';
  } catch {
    // Outside Tauri or command not available — hide XEL
    isWindows.value = false;
  }
});
</script>

<template>
  <div class="w-screen h-screen flex flex-col bg-slate-900 overflow-hidden">
    <!-- Header -->
    <header class="flex items-center justify-between px-4 py-1.5 bg-gradient-to-r from-indigo-900 to-purple-900 border-b border-indigo-500">
      <h1 class="text-sm font-bold text-white tracking-tight">
        <i class="fa-solid fa-diagram-project mr-1.5 text-indigo-400"></i>
        SQL Plan For Dummies
      </h1>
      <div class="flex items-center gap-3">
        <!-- Connection Status (clickable) -->
        <button
          v-if="dbState.connected"
          @click="showConnectionDialog = true"
          class="flex items-center gap-2 px-3 py-1.5 bg-green-900/30 border border-green-700/50 rounded-lg text-sm text-green-300 hover:bg-green-900/50 transition-colors cursor-pointer"
        >
          <span class="w-2 h-2 rounded-full bg-green-400 animate-pulse"></span>
          {{ dbState.activeConnection?.name || 'Connected' }}
        </button>
        <button
          v-else
          @click="showConnectionDialog = true"
          class="flex items-center gap-2 px-3 py-1.5 bg-slate-800/50 rounded-lg text-sm text-slate-500 hover:bg-slate-700/50 hover:text-slate-300 transition-colors cursor-pointer"
        >
          <span class="w-2 h-2 rounded-full bg-slate-600"></span>
          Disconnected
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

    <!-- Tab Navigation -->
    <nav class="flex bg-slate-800 border-b border-slate-700 px-6">
      <router-link
        to="/plan-viewer"
        class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
        :class="$route.path === '/plan-viewer'
          ? 'text-indigo-300 border-indigo-400'
          : 'text-slate-400 border-transparent hover:text-slate-300 hover:border-slate-600'"
      >
        <i class="fa-solid fa-diagram-project mr-2"></i>
        Plan Viewer
      </router-link>
      <router-link
        to="/sql-editor"
        class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
        :class="$route.path === '/sql-editor'
          ? 'text-indigo-300 border-indigo-400'
          : 'text-slate-400 border-transparent hover:text-slate-300 hover:border-slate-600'"
      >
        <i class="fa-solid fa-code mr-2"></i>
        SQL Editor
      </router-link>
      <router-link
        v-if="isWindows"
        to="/xel-analyzer"
        class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px"
        :class="$route.path === '/xel-analyzer'
          ? 'text-indigo-300 border-indigo-400'
          : 'text-slate-400 border-transparent hover:text-slate-300 hover:border-slate-600'"
      >
        <i class="fa-solid fa-chart-gantt mr-2"></i>
        XEL Analyzer
      </router-link>
    </nav>

    <!-- Router View -->
    <router-view class="flex-1 overflow-hidden" />

    <!-- Connection Dialog -->
    <Teleport to="body">
      <div
        v-if="showConnectionDialog"
        class="fixed inset-0 z-50 flex items-center justify-center"
      >
        <!-- Backdrop -->
        <div
          class="absolute inset-0 bg-black/60 backdrop-blur-sm"
          @click="showConnectionDialog = false"
        ></div>
        <!-- Dialog -->
        <div class="relative w-full max-w-md max-h-[80vh] m-4">
          <button
            @click="showConnectionDialog = false"
            class="absolute -top-2 -right-2 z-10 w-7 h-7 flex items-center justify-center rounded-full bg-slate-700 border border-slate-600 text-slate-400 hover:text-white hover:bg-slate-600 transition-colors shadow-lg"
          >
            <i class="fa-solid fa-xmark text-xs"></i>
          </button>
          <ConnectionManager />
        </div>
      </div>
    </Teleport>
  </div>
</template>
