<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import ExecutionPlanGraph from '../components/ExecutionPlanGraph.vue';
import NodeDetails from '../components/NodeDetails.vue';
import PlanLoader from '../components/PlanLoader.vue';
import AnalysisPanel from '../components/AnalysisPanel.vue';
import SqlViewer from '../components/SqlViewer.vue';
import PlanComparison from '../components/PlanComparison.vue';
import PlanOverview from '../components/PlanOverview.vue';
import { usePlanState } from '../composables/planState';
import { useResizePanel } from '../composables/useResizePanel';

const { state, statements, loadPlan, loadComparisonPlan, toggleComparisonMode } = usePlanState();

const containerRef = ref<HTMLElement | null>(null);
const HANDLE_WIDTH = 16;

const left = useResizePanel({
  initial: 320,
  direction: 'left',
  getMaxSize: () => {
    const total = containerRef.value?.clientWidth ?? 1200;
    const rightSize = right.collapsed.value ? 0 : right.size.value;
    return total - rightSize - HANDLE_WIDTH * 2;
  },
});
const right = useResizePanel({
  initial: 380,
  direction: 'right',
  getMaxSize: () => {
    const total = containerRef.value?.clientWidth ?? 1200;
    const leftSize = left.collapsed.value ? 0 : left.size.value;
    return total - leftSize - HANDLE_WIDTH * 2;
  },
});

type MainTab = 'execution' | 'analysis' | 'query' | 'xml' | 'overview';
const activeMainTab = ref<MainTab>('execution');

function prettyPrintXml(xml: string): string {
  let result = '';
  let depth = 0;
  const parts = xml.replace(/>\s*</g, '>\n<').split('\n');
  for (const part of parts) {
    const stripped = part.trim();
    if (!stripped) continue;
    const isClosing = /^<\//.test(stripped);
    const isSelfClosing = /\/>$/.test(stripped) || /^<!/.test(stripped) || /^<\?/.test(stripped);
    if (isClosing) depth--;
    result += '  '.repeat(Math.max(0, depth)) + stripped + '\n';
    if (!isClosing && !isSelfClosing) depth++;
  }
  return result.trimEnd();
}

const selectedStatementXml = computed(() => {
  if (!state.rawXml || !state.selectedStatement) return '';
  const parser = new DOMParser();
  const doc = parser.parseFromString(state.rawXml, 'text/xml');
  const stmtId = state.selectedStatement.statementId;
  const stmtEl = doc.querySelector(`StmtSimple[StatementId="${stmtId}"]`);
  if (!stmtEl) return prettyPrintXml(state.rawXml);
  const serializer = new XMLSerializer();
  return prettyPrintXml(serializer.serializeToString(stmtEl));
});
const analysisPanelRef = ref<InstanceType<typeof AnalysisPanel> | null>(null);

const xmlCopied = ref(false);
const copyXml = async () => {
  await navigator.clipboard.writeText(selectedStatementXml.value);
  xmlCopied.value = true;
  setTimeout(() => { xmlCopied.value = false; }, 1500);
};

// XML tab: paste a plan from the clipboard or edit the plan XML directly
const xmlEditing = ref(false);
const xmlDraft = ref('');
const xmlStatus = ref('');
const xmlError = ref(false);

const setXmlStatus = (message: string, error = false) => {
  xmlStatus.value = message;
  xmlError.value = error;
  if (!error) {
    setTimeout(() => {
      if (xmlStatus.value === message) xmlStatus.value = '';
    }, 4000);
  }
};

const loadPlanFromText = (text: string | null | undefined): boolean => {
  const content = text?.trim();
  if (!content) {
    setXmlStatus('No XML to load.', true);
    return false;
  }
  if (!content.includes('<ShowPlanXML')) {
    setXmlStatus('Not a SQL Server execution plan (ShowPlanXML element not found).', true);
    return false;
  }
  loadPlan(content);
  if (state.error) {
    setXmlStatus(`Parse error: ${state.error}`, true);
    return false;
  }
  const count = statements.value.length;
  setXmlStatus(`Loaded ${count} statement${count !== 1 ? 's' : ''}`);
  return true;
};

const pasteXmlFromClipboard = async () => {
  try {
    loadPlanFromText(await navigator.clipboard.readText());
  } catch {
    setXmlStatus('Could not read the clipboard. Press Ctrl+V instead.', true);
  }
};

const startXmlEdit = () => {
  xmlDraft.value = state.rawXml ? prettyPrintXml(state.rawXml) : '';
  xmlEditing.value = true;
};

const applyXmlEdit = () => {
  if (loadPlanFromText(xmlDraft.value)) {
    xmlEditing.value = false;
  }
};

const cancelXmlEdit = () => {
  xmlEditing.value = false;
  xmlDraft.value = '';
};

const onXmlPaste = (evt: ClipboardEvent) => {
  if (activeMainTab.value !== 'xml' || xmlEditing.value) return;
  const target = evt.target as HTMLElement | null;
  if (target?.closest('input, textarea, [contenteditable="true"]')) return;
  const text = evt.clipboardData?.getData('text');
  if (!text?.includes('<ShowPlanXML')) return;
  evt.preventDefault();
  loadPlanFromText(text);
};

onMounted(() => document.addEventListener('paste', onXmlPaste));
onBeforeUnmount(() => document.removeEventListener('paste', onXmlPaste));

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
  input.value = '';
};
</script>

<template>
  <div class="flex flex-col h-full overflow-hidden">
    <!-- Main layout -->
    <div ref="containerRef" class="flex-1 flex p-4 gap-0 overflow-hidden">
      <!-- Left Sidebar: Plan Loader -->
      <aside
        v-show="!left.collapsed.value"
        class="overflow-hidden shrink-0"
        :style="{ width: left.size.value + 'px' }"
        data-select-scope
      >
        <PlanLoader />
      </aside>

      <!-- Left Handle -->
      <div
        class="shrink-0 w-4 flex items-center justify-center cursor-col-resize z-10"
        @pointerdown="left.onPointerDown"
        @dblclick="left.onDoubleClick"
      >
        <div class="w-0.5 h-8 rounded-full bg-slate-600"></div>
      </div>

      <!-- Main: Tabbed panel -->
      <main class="overflow-hidden flex flex-col bg-slate-800 rounded-2xl shadow-xl flex-1 min-w-0">
        <!-- Tab bar -->
        <div class="flex items-center overflow-x-auto bg-slate-700 border-b border-slate-600 rounded-t-2xl shrink-0">
          <button
            class="flex shrink-0 items-center gap-2 whitespace-nowrap px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'execution'
              ? 'border-blue-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'execution'"
          >
            <i class="fa-solid fa-diagram-project text-blue-400"></i>
            Execution Plan
            <span v-if="activeMainTab === 'execution' && state.selectedStatement" class="text-xs text-slate-400 font-normal ml-1">
              {{ state.selectedStatement.statementSubTreeCost.toFixed(6) }}
            </span>
          </button>
          <button
            class="flex shrink-0 items-center gap-2 whitespace-nowrap px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'analysis'
              ? 'border-purple-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'analysis'"
          >
            <i class="fa-solid fa-microscope text-purple-400"></i>
            Plan Analysis
            <span
              v-if="analysisPanelRef?.issueCount"
              class="px-1.5 py-0.5 bg-amber-500/20 text-amber-400 text-xs font-semibold rounded-full ml-1"
            >
              {{ analysisPanelRef.issueCount }}
            </span>
          </button>
          <button
            class="flex shrink-0 items-center gap-2 whitespace-nowrap px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'query'
              ? 'border-emerald-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'query'"
          >
            <i class="fa-solid fa-code text-emerald-400"></i>
            Query
          </button>
          <button
            class="flex shrink-0 items-center gap-2 whitespace-nowrap px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'xml'
              ? 'border-amber-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'xml'"
          >
            <i class="fa-solid fa-file-code text-amber-400"></i>
            XML
          </button>
          <button
            class="flex shrink-0 items-center gap-2 whitespace-nowrap px-4 py-3 text-sm font-semibold transition-colors border-b-2"
            :class="activeMainTab === 'overview'
              ? 'border-cyan-400 text-white'
              : 'border-transparent text-slate-400 hover:text-slate-200'"
            @click="activeMainTab = 'overview'"
          >
            <i class="fa-solid fa-clipboard-list text-cyan-400"></i>
            Plan Overview
          </button>
          <div class="ml-auto shrink-0 px-3">
            <input
              ref="comparisonFileInput"
              type="file"
              accept=".sqlplan,.xml"
              class="hidden"
              @change="handleComparisonFile"
            />
            <button
              v-if="state.plan"
              class="px-3 py-1.5 rounded-lg text-sm flex items-center gap-2 transition-colors"
              :class="state.comparisonMode
                ? 'bg-blue-600 text-white hover:bg-blue-500'
                : 'bg-slate-600 hover:bg-slate-500 text-slate-300'"
              @click="state.comparisonPlan ? toggleComparisonMode() : openComparisonFilePicker()"
            >
              <i class="fa-solid fa-code-compare"></i>
              {{ state.comparisonMode ? 'Hide Compare' : 'Compare Plans' }}
            </button>
          </div>
        </div>

        <!-- Tab content -->
        <div class="flex-1 overflow-hidden relative">
          <!-- Loading overlay -->
          <div v-if="state.loading" class="absolute inset-0 z-10 flex items-center justify-center bg-slate-800/80">
            <div class="flex flex-col items-center gap-3">
              <i class="fa-solid fa-spinner fa-spin text-3xl text-blue-400"></i>
              <span class="text-sm text-slate-300">Loading execution plan...</span>
            </div>
          </div>

          <div v-show="activeMainTab === 'execution'" class="absolute inset-0">
            <ExecutionPlanGraph :show-header="false" />
          </div>
          <div v-show="activeMainTab === 'analysis'" class="absolute inset-0" data-select-scope>
            <AnalysisPanel ref="analysisPanelRef" :show-header="false" />
          </div>
          <div v-show="activeMainTab === 'query'" class="absolute inset-0" data-select-scope>
            <SqlViewer v-if="state.selectedStatement" :text="state.selectedStatement.statementText" />
            <div v-else class="flex items-center justify-center h-full text-slate-500 text-sm">
              No statement selected
            </div>
          </div>
          <div v-show="activeMainTab === 'overview'" class="absolute inset-0" data-select-scope>
            <PlanOverview @statement-selected="activeMainTab = 'execution'" />
          </div>
          <div v-show="activeMainTab === 'xml'" class="absolute inset-0 flex flex-col">
            <!-- XML toolbar -->
            <div class="flex items-center gap-2 px-3 py-2 bg-slate-700/50 border-b border-slate-600 shrink-0">
              <span
                v-if="xmlStatus"
                class="flex-1 text-xs truncate"
                :class="xmlError ? 'text-red-300' : 'text-green-300'"
              >
                <i class="fa-solid mr-1" :class="xmlError ? 'fa-circle-exclamation' : 'fa-circle-check'"></i>
                {{ xmlStatus }}
              </span>
              <span v-else class="flex-1 text-xs text-slate-500">
                {{ xmlEditing ? 'Editing full plan XML' : 'Selected statement XML' }}
              </span>
              <template v-if="!xmlEditing">
                <button
                  v-if="selectedStatementXml"
                  class="px-3 py-1.5 rounded-lg text-xs flex items-center gap-1.5 transition-colors"
                  :class="xmlCopied
                    ? 'bg-green-600/30 text-green-300'
                    : 'bg-slate-600 hover:bg-slate-500 text-slate-300'"
                  @click="copyXml"
                >
                  <i class="fa-solid" :class="xmlCopied ? 'fa-check' : 'fa-copy'"></i>
                  {{ xmlCopied ? 'Copied' : 'Copy' }}
                </button>
                <button
                  class="px-3 py-1.5 rounded-lg text-xs flex items-center gap-1.5 bg-slate-600 hover:bg-slate-500 text-slate-300 transition-colors"
                  title="Load a plan from the clipboard (Ctrl+V)"
                  @click="pasteXmlFromClipboard"
                >
                  <i class="fa-solid fa-paste"></i>
                  Paste plan
                </button>
                <button
                  class="px-3 py-1.5 rounded-lg text-xs flex items-center gap-1.5 bg-slate-600 hover:bg-slate-500 text-slate-300 transition-colors"
                  title="Edit the full plan XML and reload it"
                  @click="startXmlEdit"
                >
                  <i class="fa-solid fa-pen"></i>
                  Edit
                </button>
              </template>
              <template v-else>
                <button
                  class="px-3 py-1.5 rounded-lg text-xs flex items-center gap-1.5 bg-blue-600 hover:bg-blue-500 text-white transition-colors"
                  @click="applyXmlEdit"
                >
                  <i class="fa-solid fa-check"></i>
                  Apply
                </button>
                <button
                  class="px-3 py-1.5 rounded-lg text-xs flex items-center gap-1.5 bg-slate-600 hover:bg-slate-500 text-slate-300 transition-colors"
                  @click="cancelXmlEdit"
                >
                  <i class="fa-solid fa-xmark"></i>
                  Cancel
                </button>
              </template>
            </div>

            <!-- XML editor -->
            <textarea
              v-if="xmlEditing"
              v-model="xmlDraft"
              spellcheck="false"
              placeholder="Paste SQL Server execution plan XML here"
              class="flex-1 w-full bg-slate-900/60 p-4 text-xs font-mono text-slate-300 leading-relaxed resize-none outline-none"
            ></textarea>

            <!-- XML viewer -->
            <div v-else class="flex-1 overflow-auto" data-select-scope>
              <pre v-if="selectedStatementXml" class="p-4 text-xs font-mono text-slate-300 leading-relaxed whitespace-pre">{{ selectedStatementXml }}</pre>
              <div v-else class="flex flex-col items-center justify-center h-full text-slate-500 text-sm gap-2">
                <p>No plan loaded</p>
                <p class="text-xs text-slate-600">Paste one from the clipboard (Ctrl+V) or click Edit to type XML</p>
              </div>
            </div>
          </div>
        </div>
      </main>

      <!-- Right Handle -->
      <div
        class="shrink-0 w-4 flex items-center justify-center cursor-col-resize z-10"
        @pointerdown="right.onPointerDown"
        @dblclick="right.onDoubleClick"
      >
        <div class="w-0.5 h-8 rounded-full bg-slate-600"></div>
      </div>

      <!-- Right Sidebar: Node Details or Comparison -->
      <aside
        v-show="!right.collapsed.value"
        class="overflow-hidden shrink-0"
        :style="{ width: right.size.value + 'px' }"
        data-select-scope
      >
        <PlanComparison v-if="state.comparisonMode && state.comparisonPlan" />
        <NodeDetails v-else />
      </aside>
    </div>
  </div>
</template>
