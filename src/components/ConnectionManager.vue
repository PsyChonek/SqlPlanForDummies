<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useDbConnection } from '../composables/useDbConnection';

const {
  state,
  connect,
  connectSaved,
  disconnect,
  testConnection,
  loadConnections,
  saveConnection,
  deleteConnection,
} = useDbConnection();

const showForm = ref(false);
const testResult = ref<'success' | 'fail' | null>(null);
const testLoading = ref(false);

// Form fields
const form = ref({
  name: '',
  host: 'localhost',
  port: 1433,
  database: '',
  username: '',
  password: '',
});

const resetForm = () => {
  form.value = { name: '', host: 'localhost', port: 1433, database: '', username: '', password: '' };
  testResult.value = null;
};

const handleTest = async () => {
  testLoading.value = true;
  testResult.value = null;
  const ok = await testConnection(
    form.value.host,
    form.value.port,
    form.value.database,
    form.value.username,
    form.value.password
  );
  testResult.value = ok ? 'success' : 'fail';
  testLoading.value = false;
};

const handleSaveAndConnect = async () => {
  try {
    const saved = await saveConnection(
      form.value.name || `${form.value.host}/${form.value.database}`,
      form.value.host,
      form.value.port,
      form.value.database,
      form.value.username,
      form.value.password
    );
    await connect(
      form.value.host,
      form.value.port,
      form.value.database,
      form.value.username,
      form.value.password,
      saved.name
    );
    showForm.value = false;
    resetForm();
  } catch (_) {
    // Error is in state.error
  }
};

const handleConnectOnly = async () => {
  try {
    await connect(
      form.value.host,
      form.value.port,
      form.value.database,
      form.value.username,
      form.value.password,
      form.value.name || `${form.value.host}/${form.value.database}`
    );
    showForm.value = false;
    resetForm();
  } catch (_) {
    // Error is in state.error
  }
};

const handleQuickConnect = async (id: string) => {
  try {
    await connectSaved(id);
  } catch (_) {
    // Error is in state.error
  }
};

const handleDelete = async (id: string) => {
  await deleteConnection(id);
};

const formatDate = (dateStr: string | null) => {
  if (!dateStr) return 'Never';
  return new Date(dateStr).toLocaleDateString();
};

onMounted(() => {
  loadConnections();
});
</script>

<template>
  <div class="flex flex-col h-full bg-slate-800 rounded-2xl shadow-xl overflow-hidden">
    <!-- Header -->
    <div class="px-4 py-3 bg-slate-700 border-b border-slate-600">
      <h3 class="flex items-center gap-2 text-lg font-bold text-white">
        <i class="fa-solid fa-database text-blue-400"></i>
        Connections
      </h3>
    </div>

    <div class="flex-1 p-4 overflow-y-auto">
      <!-- Active Connection -->
      <div v-if="state.connected" class="mb-4 p-3 rounded-lg bg-green-900/20 border border-green-700/50">
        <div class="flex items-center justify-between">
          <div>
            <div class="text-sm font-medium text-green-300 flex items-center gap-2">
              <span class="w-2 h-2 rounded-full bg-green-400"></span>
              {{ state.activeConnection?.name }}
            </div>
            <div class="text-xs text-green-400/70 mt-1">
              {{ state.activeConnection?.host }}:{{ state.activeConnection?.port }}/{{ state.activeConnection?.database }}
            </div>
          </div>
          <button
            class="px-2 py-1 text-xs rounded bg-red-900/50 text-red-300 hover:bg-red-800/50 transition-colors"
            @click="disconnect"
          >
            Disconnect
          </button>
        </div>
      </div>

      <!-- New Connection Button -->
      <button
        v-if="!showForm"
        class="w-full mb-4 px-3 py-2 rounded-lg text-sm font-medium bg-indigo-600 hover:bg-indigo-500 text-white transition-colors flex items-center justify-center gap-2"
        @click="showForm = true"
      >
        <i class="fa-solid fa-plus"></i>
        New Connection
      </button>

      <!-- Connection Form -->
      <div v-if="showForm" class="mb-4 p-3 rounded-lg bg-slate-700/50 border border-slate-600 space-y-3">
        <div>
          <label class="block text-xs font-medium text-slate-400 mb-1">Name</label>
          <input
            v-model="form.name"
            type="text"
            placeholder="My Server"
            class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
          />
        </div>
        <div class="grid grid-cols-3 gap-2">
          <div class="col-span-2">
            <label class="block text-xs font-medium text-slate-400 mb-1">Host</label>
            <input
              v-model="form.host"
              type="text"
              class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
            />
          </div>
          <div>
            <label class="block text-xs font-medium text-slate-400 mb-1">Port</label>
            <input
              v-model.number="form.port"
              type="number"
              class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
            />
          </div>
        </div>
        <div>
          <label class="block text-xs font-medium text-slate-400 mb-1">Database</label>
          <input
            v-model="form.database"
            type="text"
            class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
          />
        </div>
        <div>
          <label class="block text-xs font-medium text-slate-400 mb-1">Username</label>
          <input
            v-model="form.username"
            type="text"
            class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
          />
        </div>
        <div>
          <label class="block text-xs font-medium text-slate-400 mb-1">Password</label>
          <input
            v-model="form.password"
            type="password"
            class="w-full px-3 py-1.5 rounded-lg bg-slate-700 text-white text-sm border border-slate-600 focus:outline-none focus:border-indigo-500"
          />
        </div>

        <!-- Test Result -->
        <div v-if="testResult === 'success'" class="text-xs text-green-400 flex items-center gap-1">
          <i class="fa-solid fa-check-circle"></i> Connection successful
        </div>
        <div v-if="testResult === 'fail'" class="text-xs text-red-400 flex items-center gap-1">
          <i class="fa-solid fa-times-circle"></i> {{ state.error || 'Connection failed' }}
        </div>

        <!-- Error -->
        <div v-if="state.error && !testResult" class="text-xs text-red-400">
          {{ state.error }}
        </div>

        <!-- Actions -->
        <div class="flex gap-2">
          <button
            class="flex-1 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-600 hover:bg-slate-500 text-slate-200 transition-colors"
            :disabled="testLoading"
            @click="handleTest"
          >
            <i :class="testLoading ? 'fa-solid fa-spinner fa-spin' : 'fa-solid fa-plug'"></i>
            Test
          </button>
          <button
            class="flex-1 px-3 py-1.5 rounded-lg text-xs font-medium bg-blue-600 hover:bg-blue-500 text-white transition-colors"
            :disabled="state.loading"
            @click="handleConnectOnly"
          >
            Connect
          </button>
          <button
            class="flex-1 px-3 py-1.5 rounded-lg text-xs font-medium bg-indigo-600 hover:bg-indigo-500 text-white transition-colors"
            :disabled="state.loading"
            @click="handleSaveAndConnect"
          >
            Save & Connect
          </button>
        </div>
        <button
          class="w-full text-xs text-slate-500 hover:text-slate-300 transition-colors"
          @click="showForm = false; resetForm()"
        >
          Cancel
        </button>
      </div>

      <!-- Saved Connections -->
      <div v-if="state.connections.length > 0">
        <h4 class="text-sm font-semibold text-slate-400 mb-2 flex items-center gap-2">
          <i class="fa-solid fa-clock-rotate-left"></i>
          Saved Connections
        </h4>
        <div class="space-y-2">
          <div
            v-for="conn in state.connections"
            :key="conn.id"
            class="p-2 rounded-lg bg-slate-700/50 border border-slate-600 hover:border-slate-500 transition-colors"
          >
            <div class="flex items-center justify-between">
              <div class="flex-1 min-w-0">
                <div class="text-sm font-medium text-slate-200 truncate">{{ conn.name }}</div>
                <div class="text-xs text-slate-500 truncate">
                  {{ conn.host }}:{{ conn.port }}/{{ conn.database }}
                </div>
                <div class="text-xs text-slate-600">Last used: {{ formatDate(conn.lastUsed) }}</div>
              </div>
              <div class="flex items-center gap-1 ml-2">
                <button
                  class="px-2 py-1 text-xs rounded bg-blue-900/50 text-blue-300 hover:bg-blue-800/50 transition-colors"
                  :disabled="state.loading"
                  @click="handleQuickConnect(conn.id)"
                >
                  <i class="fa-solid fa-plug"></i>
                </button>
                <button
                  class="px-2 py-1 text-xs rounded bg-red-900/50 text-red-400 hover:bg-red-800/50 transition-colors"
                  @click="handleDelete(conn.id)"
                >
                  <i class="fa-solid fa-trash"></i>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
