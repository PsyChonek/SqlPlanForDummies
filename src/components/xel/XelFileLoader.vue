<script setup lang="ts">
import { ref, reactive, nextTick, onMounted, onUnmounted } from 'vue';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';
import { useXelState } from '../../composables/useXelState';
import * as xelApi from '../../composables/xelTauriApi';
import type { XelLoadProgress, PowerShellStatus, XelEnrichResult } from '../../types/xel';

const { state, addFile, setStats, setLoading, setError, clearAll, selectEvent } = useXelState();

const enriching = ref(false);
const enrichResult = ref<XelEnrichResult | null>(null);

const enrichFromDb = async () => {
  enriching.value = true;
  enrichResult.value = null;
  try {
    enrichResult.value = await xelApi.enrichFromDb();
    // Refresh selected event to pick up resolved names
    if (state.selectedEvent) {
      const eventId = state.selectedEvent.id;
      const refreshed = await xelApi.getEvent(eventId);
      if (refreshed) {
        // Force watcher to re-fire: clear, tick, then re-set
        selectEvent(null);
        await nextTick();
        selectEvent(refreshed);
      }
    }
    // Refresh stats to pick up new database names etc
    state.revision++;
  } catch (err: unknown) {
    enrichResult.value = {
      databasesResolved: 0,
      objectsResolved: 0,
      queryTextsResolved: 0,
      uniqueDatabases: 0,
      uniqueObjects: 0,
      uniqueQueries: 0,
      errors: [err instanceof Error ? err.message : String(err)],
    };
  } finally {
    enriching.value = false;
  }
};

const dragOver = ref(false);
const psStatus = ref<PowerShellStatus | null>(null);

let unlistenDrop: (() => void) | null = null;

// Per-file progress tracking
interface FileProgress {
  name: string;
  path: string;
  eventsParsed: number;
  phase: string;
  done: boolean;
}
const fileProgressMap = reactive<Map<string, FileProgress>>(new Map());

const checkPs = async () => {
  if (psStatus.value) return;
  try {
    psStatus.value = await xelApi.checkPowerShell();
  } catch {
    psStatus.value = { available: false, sqlServerModule: false, dbatoolsModule: false, message: 'Could not check PowerShell' };
  }
};

const loadFiles = async (paths: string[]) => {
  const alreadyLoaded = new Set(state.files.map(f => f.path));
  const validPaths = paths.filter(p => (p.endsWith('.xel') || p.endsWith('.xml')) && !alreadyLoaded.has(p));
  if (validPaths.length === 0) return;

  await checkPs();
  setLoading(true);
  setError(null);

  // Initialize per-file progress
  fileProgressMap.clear();
  for (const p of validPaths) {
    const name = p.split(/[/\\]/).pop() || p;
    fileProgressMap.set(name, { name, path: p, eventsParsed: 0, phase: 'waiting', done: false });
  }

  const unlisten = await listen<XelLoadProgress>('xel-load-progress', (event) => {
    const p = event.payload;
    const fileName = p.fileName;

    // Find matching file entry
    const entry = fileProgressMap.get(fileName);
    if (entry) {
      entry.eventsParsed = p.eventsParsed;
      entry.phase = p.phase;
      entry.done = p.phase === 'complete';
    } else if (fileName !== 'all') {
      // Try matching by partial name
      for (const [key, val] of fileProgressMap.entries()) {
        if (fileName.includes(key) || key.includes(fileName)) {
          val.eventsParsed = p.eventsParsed;
          val.phase = p.phase;
          val.done = p.phase === 'complete';
          break;
        }
      }
    }
  });

  try {
    const stats = await xelApi.loadXelFiles({ filePaths: validPaths, append: state.files.length > 0 });

    for (const path of validPaths) {
      const name = path.split(/[/\\]/).pop() || path;
      addFile({ path, name, sizeBytes: 0, eventCount: stats.totalEvents });
    }

    setStats(stats);
    fileProgressMap.clear();
  } catch (err: unknown) {
    setError(err instanceof Error ? err.message : String(err));
  } finally {
    setLoading(false);
    unlisten();
  }
};

onMounted(async () => {
  try {
    const appWindow = getCurrentWebviewWindow();
    unlistenDrop = await appWindow.onDragDropEvent((event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        dragOver.value = true;
      } else if (event.payload.type === 'leave') {
        dragOver.value = false;
      } else if (event.payload.type === 'drop') {
        dragOver.value = false;
        const paths = event.payload.paths;
        if (paths.length > 0) {
          loadFiles(paths);
        }
      }
    });
  } catch (err) {
    console.warn('Tauri drag-drop not available:', err);
  }
});

onUnmounted(() => {
  unlistenDrop?.();
});

const openFilePicker = async () => {
  try {
    const paths = await xelApi.pickFiles();
    if (paths.length > 0) {
      await loadFiles(paths);
    }
  } catch (err: unknown) {
    setError(err instanceof Error ? err.message : String(err));
  }
};

const handleClear = async () => {
  try {
    await xelApi.clearXelData();
    clearAll();
    fileProgressMap.clear();
  } catch (err: unknown) {
    setError(err instanceof Error ? err.message : String(err));
  }
};

const phaseLabel = (phase: string): string => {
  switch (phase) {
    case 'starting': return 'Starting...';
    case 'checkingPowerShell': return 'Setting up...';
    case 'parsing': return 'Parsing';
    case 'indexing': return 'Indexing...';
    case 'complete': return 'Done';
    case 'error': return 'Error';
    case 'waiting': return 'Waiting...';
    default: return phase;
  }
};
</script>

<template>
  <div class="flex flex-col h-full gap-3">
    <!-- Drop zone -->
    <div
      class="flex flex-col items-center justify-center px-4 py-6 border-2 border-dashed rounded-xl cursor-pointer transition-colors"
      :class="dragOver
        ? 'border-indigo-400 bg-indigo-900/20'
        : 'border-slate-600 hover:border-slate-500 bg-slate-800/50'"
      @click="openFilePicker"
    >
      <i class="fa-solid fa-file-import text-2xl mb-2" :class="dragOver ? 'text-indigo-400' : 'text-slate-500'"></i>
      <p class="text-sm font-medium" :class="dragOver ? 'text-indigo-300' : 'text-slate-400'">
        {{ dragOver ? 'Drop files here' : 'Drop .xel file or click to browse' }}
      </p>
    </div>

    <!-- Per-file progress during loading -->
    <div v-if="fileProgressMap.size > 0" class="flex flex-col min-h-0">
      <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-1.5 shrink-0">Processing</h3>
      <div class="overflow-auto space-y-1.5">
      <div
        v-for="[key, fp] of fileProgressMap"
        :key="key"
        class="px-2 py-1.5 rounded-lg text-xs"
        :class="fp.done ? 'bg-green-900/20 border border-green-700/30' : 'bg-indigo-900/20 border border-indigo-700/30'"
      >
        <div class="flex items-center gap-2">
          <i
            :class="fp.done ? 'fa-solid fa-circle-check text-green-400' : 'fa-solid fa-spinner fa-spin text-indigo-400'"
          ></i>
          <span class="truncate flex-1 text-slate-300" :title="fp.path">{{ fp.name }}</span>
          <span class="shrink-0 text-slate-400">
            <template v-if="fp.phase === 'parsing' && fp.eventsParsed > 0">
              {{ fp.eventsParsed.toLocaleString() }} events
            </template>
            <template v-else>
              {{ phaseLabel(fp.phase) }}
            </template>
          </span>
        </div>
      </div>
      </div>
    </div>

    <!-- Error -->
    <div v-if="state.error" class="px-3 py-2 bg-red-900/20 border border-red-700/30 rounded-lg">
      <p class="text-xs text-red-400 break-words">{{ state.error }}</p>
    </div>

    <!-- PS Status -->
    <div
      v-if="psStatus && !psStatus.sqlServerModule && !psStatus.dbatoolsModule"
      class="px-3 py-2 bg-yellow-900/20 border border-yellow-700/30 rounded-lg"
    >
      <p class="text-xs text-yellow-400">{{ psStatus.message }}</p>
    </div>

    <!-- Loaded files -->
    <div v-if="state.files.length > 0 && fileProgressMap.size === 0" class="flex-1 overflow-auto">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Loaded Files</h3>
        <button @click="handleClear" class="text-xs text-slate-500 hover:text-red-400 transition-colors">
          Clear All
        </button>
      </div>
      <div class="space-y-1">
        <div
          v-for="file in state.files"
          :key="file.path"
          class="flex items-center gap-2 px-2 py-1.5 bg-slate-700/50 rounded-lg text-sm"
        >
          <i class="fa-solid fa-circle-check text-green-500 text-xs"></i>
          <span class="text-slate-300 truncate flex-1" :title="file.path">{{ file.name }}</span>
        </div>
      </div>
    </div>

    <!-- Enrich from DB -->
    <div v-if="state.stats && state.stats.totalEvents > 0 && fileProgressMap.size === 0" class="border-t border-slate-700 pt-2">
      <button
        @click="enrichFromDb"
        :disabled="enriching"
        class="w-full px-3 py-2 rounded-lg text-xs font-medium transition-colors flex items-center justify-center gap-2"
        :class="enriching
          ? 'bg-slate-700 text-slate-400 cursor-wait'
          : 'bg-emerald-700/50 hover:bg-emerald-600/50 text-emerald-300 border border-emerald-700/50'"
      >
        <i :class="enriching ? 'fa-solid fa-spinner fa-spin' : 'fa-solid fa-database'"></i>
        {{ enriching ? 'Enriching...' : 'Enrich from DB' }}
      </button>
      <p class="text-[10px] text-slate-500 mt-1 text-center">
        Resolve database names, object names, query texts from connected DB
      </p>
      <!-- Enrich result -->
      <div v-if="enrichResult" class="mt-1.5 px-2 py-1.5 rounded text-[10px]"
        :class="enrichResult.errors.length > 0 ? 'bg-yellow-900/20 border border-yellow-700/30' : 'bg-green-900/20 border border-green-700/30'"
      >
        <div class="space-y-0.5">
          <div v-if="enrichResult.databasesResolved > 0" class="text-green-400">
            {{ enrichResult.uniqueDatabases }} DB name{{ enrichResult.uniqueDatabases !== 1 ? 's' : '' }} resolved
            <span class="text-slate-500">({{ enrichResult.databasesResolved.toLocaleString() }} events updated)</span>
          </div>
          <div v-if="enrichResult.objectsResolved > 0" class="text-green-400">
            {{ enrichResult.uniqueObjects }} object{{ enrichResult.uniqueObjects !== 1 ? 's' : '' }} resolved
            <span class="text-slate-500">({{ enrichResult.objectsResolved.toLocaleString() }} events updated)</span>
          </div>
          <div v-if="enrichResult.queryTextsResolved > 0" class="text-green-400">
            {{ enrichResult.uniqueQueries }} quer{{ enrichResult.uniqueQueries !== 1 ? 'ies' : 'y' }} from Query Store
            <span class="text-slate-500">({{ enrichResult.queryTextsResolved.toLocaleString() }} events updated)</span>
          </div>
          <div v-if="enrichResult.databasesResolved === 0 && enrichResult.objectsResolved === 0 && enrichResult.queryTextsResolved === 0" class="text-slate-400">
            Nothing to resolve (connect to the source database)
          </div>
        </div>
        <div v-for="(err, i) in enrichResult.errors" :key="i" class="text-yellow-400 mt-0.5">{{ err }}</div>
      </div>
    </div>

    <!-- Stats summary -->
    <div v-if="state.stats && fileProgressMap.size === 0" class="space-y-1 text-xs text-slate-400 border-t border-slate-700 pt-2">
      <div class="flex justify-between">
        <span>Events</span>
        <span class="text-slate-300 font-medium">{{ state.stats.totalEvents.toLocaleString() }}</span>
      </div>
      <div class="flex justify-between">
        <span>Sessions</span>
        <span class="text-slate-300 font-medium">{{ state.stats.uniqueSessions.length }}</span>
      </div>
      <div v-if="state.stats.timeRangeStart" class="flex justify-between">
        <span>Time Range</span>
        <span class="text-slate-300 font-medium text-right">
          {{ new Date(state.stats.timeRangeStart).toLocaleString('en-US', { month: 'short', day: 'numeric', hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) }}
          -
          {{ new Date(state.stats.timeRangeEnd!).toLocaleString('en-US', { month: 'short', day: 'numeric', hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) }}
        </span>
      </div>
    </div>
  </div>
</template>
