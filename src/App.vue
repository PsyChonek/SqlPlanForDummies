<script setup lang="ts">
import { useDbConnection } from './composables/useDbConnection';

const { state: dbState } = useDbConnection();
</script>

<template>
  <div class="w-screen h-screen flex flex-col bg-slate-900 overflow-hidden">
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
        <!-- Connection Status -->
        <div
          v-if="dbState.connected"
          class="flex items-center gap-2 px-3 py-1.5 bg-green-900/30 border border-green-700/50 rounded-lg text-sm text-green-300"
        >
          <span class="w-2 h-2 rounded-full bg-green-400 animate-pulse"></span>
          {{ dbState.activeConnection?.name || 'Connected' }}
        </div>
        <div
          v-else
          class="flex items-center gap-2 px-3 py-1.5 bg-slate-800/50 rounded-lg text-sm text-slate-500"
        >
          <span class="w-2 h-2 rounded-full bg-slate-600"></span>
          Disconnected
        </div>

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
    </nav>

    <!-- Router View -->
    <router-view class="flex-1 overflow-hidden" />
  </div>
</template>
