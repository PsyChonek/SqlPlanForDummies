<script setup lang="ts">
import { ref } from 'vue';
import { usePlanState } from '../composables/planState';

const { state, loadPlan, statements, selectStatement } = usePlanState();

const fileInput = ref<HTMLInputElement>();
const dragActive = ref(false);
const statusMessage = ref<string>('');
const isError = ref(false);

const openFileSelector = () => {
  fileInput.value?.click();
};

const handleFileSelect = async (evt: Event) => {
  const input = evt.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) {
    await processFile(file);
  }
  // Reset input so same file can be selected again
  input.value = '';
};

const onDragEnter = (evt: DragEvent) => {
  evt.preventDefault();
  dragActive.value = true;
};

const onDragLeave = () => {
  dragActive.value = false;
};

const onDrop = async (evt: DragEvent) => {
  evt.preventDefault();
  dragActive.value = false;
  
  const file = evt.dataTransfer?.files[0];
  if (file) {
    await processFile(file);
  }
};

const processFile = async (file: File) => {
  isError.value = false;
  
  // Validate file extension
  const fileName = file.name.toLowerCase();
  if (!fileName.endsWith('.sqlplan') && !fileName.endsWith('.xml')) {
    setStatus('Invalid file type. Please use .sqlplan or .xml files.', true);
    return;
  }
  
  try {
    setStatus('Reading file...');
    const content = await file.text();
    
    setStatus('Parsing execution plan...');
    loadPlan(content);
    
    if (state.error) {
      setStatus(`Parse error: ${state.error}`, true);
    } else {
      const stmtCount = statements.value.length;
      setStatus(`Loaded ${stmtCount} statement${stmtCount !== 1 ? 's' : ''} successfully`);
    }
  } catch (err) {
    setStatus(`Error: ${err instanceof Error ? err.message : 'Failed to read file'}`, true);
  }
};

const setStatus = (message: string, error = false) => {
  statusMessage.value = message;
  isError.value = error;
  
  if (!error) {
    setTimeout(() => {
      if (statusMessage.value === message) {
        statusMessage.value = '';
      }
    }, 4000);
  }
};
</script>

<template>
  <div class="flex flex-col h-full bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div class="px-4 py-3 bg-slate-700 border-b border-slate-600">
      <h3 class="flex items-center gap-2 text-lg font-bold text-white">
        <i class="fa-solid fa-file-import text-blue-400"></i>
        SQL Plan Loader
      </h3>
    </div>
    
    <div class="flex-1 p-4 overflow-y-auto">
      <!-- Drop Zone -->
      <div 
        class="relative border-2 border-dashed rounded-xl p-8 text-center cursor-pointer transition-all duration-300"
        :class="[
          dragActive 
            ? 'border-blue-400 bg-blue-500/20 scale-[1.02]' 
            : 'border-slate-600 bg-slate-700/50 hover:border-slate-500 hover:bg-slate-700'
        ]"
        @click="openFileSelector"
        @dragover.prevent="onDragEnter"
        @dragenter.prevent="onDragEnter"
        @dragleave="onDragLeave"
        @drop="onDrop"
      >
        <div class="mb-4">
          <i class="fa-solid fa-file-code text-5xl" :class="dragActive ? 'text-blue-400' : 'text-slate-500'"></i>
        </div>
        <p class="text-white font-semibold mb-1">
          {{ dragActive ? 'Drop file here' : 'Drop .sqlplan file or click to browse' }}
        </p>
        <p class="text-sm text-slate-400">
          SQL Server Execution Plan XML
        </p>
      </div>
      
      <input
        ref="fileInput"
        type="file"
        accept=".sqlplan,.xml"
        class="hidden"
        @change="handleFileSelect"
      />
      
      <!-- Status Message -->
      <div 
        v-if="statusMessage"
        class="mt-4 px-4 py-3 rounded-lg text-sm font-medium flex items-center gap-2"
        :class="isError ? 'bg-red-500/20 border border-red-500/50 text-red-300' : 'bg-green-500/20 border border-green-500/50 text-green-300'"
      >
        <i :class="isError ? 'fa-solid fa-circle-exclamation' : 'fa-solid fa-circle-check'"></i>
        {{ statusMessage }}
      </div>
      
      <!-- Statement List -->
      <div v-if="statements.length > 0" class="mt-6">
        <h4 class="text-sm font-semibold text-slate-400 mb-2 flex items-center gap-2">
          <i class="fa-solid fa-list-ol"></i>
          Statements ({{ statements.length }})
        </h4>
        
        <div class="space-y-2">
          <button
            v-for="(stmt, idx) in statements"
            :key="stmt.statementId"
            class="w-full text-left px-3 py-2 rounded-lg transition-colors"
            :class="state.selectedStatement?.statementId === stmt.statementId 
              ? 'bg-blue-600 text-white' 
              : 'bg-slate-700 hover:bg-slate-600 text-slate-300'"
            @click="selectStatement(stmt)"
          >
            <div class="flex items-center justify-between">
              <span class="font-medium text-sm">
                <i class="fa-solid fa-code mr-1"></i>
                Statement {{ idx + 1 }}
              </span>
              <span class="text-xs opacity-75">
                Cost: {{ stmt.statementSubTreeCost.toFixed(4) }}
              </span>
            </div>
            <p class="text-xs mt-1 truncate opacity-75">
              {{ stmt.statementType }}: {{ stmt.statementText.substring(0, 50) }}...
            </p>
          </button>
        </div>
      </div>
    </div>
    
    <!-- Footer info -->
    <div class="px-4 py-2 bg-slate-700/50 border-t border-slate-600">
      <p class="text-xs text-slate-500 flex items-center gap-1">
        <i class="fa-solid fa-info-circle"></i>
        Supports SQL Server ShowPlan XML format
      </p>
    </div>
  </div>
</template>
