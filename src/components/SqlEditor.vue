<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { EditorView, keymap, placeholder } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { sql, MSSQL } from '@codemirror/lang-sql';
import { oneDark } from '@codemirror/theme-one-dark';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { useQueryExecution, type PlanType } from '../composables/useQueryExecution';
import { useDbConnection } from '../composables/useDbConnection';
import { useQueryHistory } from '../composables/useQueryHistory';

interface EditorTab {
  id: string;
  title: string;
  content: string;
  view: EditorView | null;
}

const { state: execState, executeQuery } = useQueryExecution();
const { state: dbState } = useDbConnection();
const { addQueryEntry, addPlanEntry } = useQueryHistory();

const editorContainer = ref<HTMLDivElement | null>(null);
const tabs = ref<EditorTab[]>([
  { id: crypto.randomUUID(), title: 'Query 1', content: '', view: null },
]);
const activeTabId = ref(tabs.value[0].id);
const planType = ref<PlanType>('Estimated');
let tabCounter = 1;

const activeTab = () => tabs.value.find((t) => t.id === activeTabId.value);

const createEditorView = (doc: string, container: HTMLElement): EditorView => {
  const executeKeymap = keymap.of([
    {
      key: 'Ctrl-e',
      run: () => {
        handleExecute();
        return true;
      },
    },
    {
      key: 'F5',
      run: () => {
        handleExecute();
        return true;
      },
    },
  ]);

  const state = EditorState.create({
    doc,
    extensions: [
      sql({ dialect: MSSQL }),
      oneDark,
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      executeKeymap,
      placeholder('Enter SQL query here...'),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          const tab = activeTab();
          if (tab) {
            tab.content = update.state.doc.toString();
          }
        }
      }),
      EditorView.theme({
        '&': { height: '100%', fontSize: '14px' },
        '.cm-scroller': { overflow: 'auto' },
        '.cm-content': { fontFamily: "'Cascadia Code', 'Fira Code', monospace" },
      }),
    ],
  });

  return new EditorView({ state, parent: container });
};

const mountEditor = () => {
  if (!editorContainer.value) return;
  const tab = activeTab();
  if (!tab) return;

  // Destroy previous view
  tabs.value.forEach((t) => {
    if (t.view) {
      t.view.destroy();
      t.view = null;
    }
  });

  editorContainer.value.innerHTML = '';
  tab.view = createEditorView(tab.content, editorContainer.value);
};

const switchTab = (tabId: string) => {
  const current = activeTab();
  if (current?.view) {
    current.content = current.view.state.doc.toString();
    current.view.destroy();
    current.view = null;
  }
  activeTabId.value = tabId;
  nextTick(mountEditor);
};

const addTab = () => {
  tabCounter++;
  const newTab: EditorTab = {
    id: crypto.randomUUID(),
    title: `Query ${tabCounter}`,
    content: '',
    view: null,
  };
  tabs.value.push(newTab);
  switchTab(newTab.id);
};

const closeTab = (tabId: string) => {
  if (tabs.value.length <= 1) return;
  const idx = tabs.value.findIndex((t) => t.id === tabId);
  const tab = tabs.value[idx];
  if (tab.view) {
    tab.view.destroy();
    tab.view = null;
  }
  tabs.value.splice(idx, 1);
  if (activeTabId.value === tabId) {
    switchTab(tabs.value[Math.min(idx, tabs.value.length - 1)].id);
  }
};

const getSelectedOrFullText = (): string => {
  const tab = activeTab();
  if (!tab?.view) return '';
  const selection = tab.view.state.selection.main;
  if (selection.from !== selection.to) {
    return tab.view.state.sliceDoc(selection.from, selection.to);
  }
  return tab.view.state.doc.toString();
};

const handleExecute = async () => {
  if (!dbState.connected) return;
  if (execState.executing) return;

  const sqlText = getSelectedOrFullText().trim();
  if (!sqlText) return;

  const queryId = crypto.randomUUID();
  const connectionId = dbState.activeConnection?.id || '';
  const connectionName = dbState.activeConnection?.name || '';

  try {
    const result = await executeQuery(sqlText, planType.value);

    await addQueryEntry({
      id: queryId,
      sql: sqlText,
      connectionId,
      connectionName,
      executedAt: new Date().toISOString(),
      durationMs: result.durationMs,
      success: true,
      error: null,
    });

    if (result.planXml) {
      await addPlanEntry({
        id: crypto.randomUUID(),
        queryId,
        planXml: result.planXml,
        planType: planType.value,
        executedAt: new Date().toISOString(),
        connectionId,
        sqlPreview: sqlText.substring(0, 100),
      });
    }
  } catch (e) {
    await addQueryEntry({
      id: queryId,
      sql: sqlText,
      connectionId,
      connectionName,
      executedAt: new Date().toISOString(),
      durationMs: 0,
      success: false,
      error: String(e),
    });
  }
};

onMounted(() => {
  nextTick(mountEditor);
});

onUnmounted(() => {
  tabs.value.forEach((t) => {
    if (t.view) {
      t.view.destroy();
      t.view = null;
    }
  });
});
</script>

<template>
  <div class="flex flex-col h-full bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Toolbar -->
    <div class="flex items-center justify-between px-4 py-2 bg-slate-700 border-b border-slate-600">
      <div class="flex items-center gap-2">
        <button
          class="px-3 py-1.5 rounded-lg text-sm font-medium flex items-center gap-2 transition-colors"
          :class="dbState.connected && !execState.executing
            ? 'bg-green-600 hover:bg-green-500 text-white'
            : 'bg-slate-600 text-slate-400 cursor-not-allowed'"
          :disabled="!dbState.connected || execState.executing"
          @click="handleExecute"
        >
          <i :class="execState.executing ? 'fa-solid fa-spinner fa-spin' : 'fa-solid fa-play'"></i>
          {{ execState.executing ? 'Executing...' : 'Execute' }}
        </button>

        <span class="text-slate-500 text-xs">Ctrl+E / F5</span>

        <!-- Plan Type Toggle -->
        <select
          v-model="planType"
          class="ml-4 px-3 py-1.5 rounded-lg text-sm bg-slate-600 text-slate-200 border border-slate-500 focus:outline-none focus:border-indigo-500"
        >
          <option value="None">No Plan</option>
          <option value="Estimated">Estimated Plan</option>
          <option value="Actual">Actual Plan</option>
        </select>
      </div>

      <div v-if="!dbState.connected" class="text-sm text-amber-400 flex items-center gap-2">
        <i class="fa-solid fa-triangle-exclamation"></i>
        Connect to a database first
      </div>
    </div>

    <!-- Editor Tabs -->
    <div class="flex items-center bg-slate-800 border-b border-slate-600 px-2">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="group flex items-center gap-2 px-3 py-1.5 text-sm border-b-2 -mb-px transition-colors"
        :class="activeTabId === tab.id
          ? 'text-indigo-300 border-indigo-400 bg-slate-700/50'
          : 'text-slate-400 border-transparent hover:text-slate-300'"
        @click="switchTab(tab.id)"
      >
        <i class="fa-solid fa-file-code text-xs"></i>
        {{ tab.title }}
        <span
          v-if="tabs.length > 1"
          class="ml-1 text-xs text-slate-500 hover:text-red-400 transition-colors"
          @click.stop="closeTab(tab.id)"
        >
          <i class="fa-solid fa-xmark"></i>
        </span>
      </button>
      <button
        class="px-2 py-1.5 text-slate-500 hover:text-slate-300 transition-colors text-sm"
        @click="addTab"
      >
        <i class="fa-solid fa-plus"></i>
      </button>
    </div>

    <!-- CodeMirror Container -->
    <div ref="editorContainer" class="flex-1 overflow-hidden"></div>
  </div>
</template>
