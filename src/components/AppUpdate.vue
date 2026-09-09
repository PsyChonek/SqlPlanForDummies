<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useAppUpdate } from '../composables/useAppUpdate';

const { status, info, error, isOpen, check, install, restart, close } = useAppUpdate();
const dialog = ref<HTMLDialogElement | null>(null);
const trigger = ref<HTMLButtonElement | null>(null);
let previousFocus: HTMLElement | null = null;
const appVersion = __APP_VERSION__;
const title = computed(() => {
  if (status.value === 'available') return 'Update available';
  if (status.value === 'installing') return 'Installing update';
  if (status.value === 'installed') return 'Update installed';
  return 'App updates';
});

watch(isOpen, (open) => {
  if (open) {
    previousFocus = document.activeElement instanceof HTMLElement && document.activeElement !== document.body
      ? document.activeElement : null;
    dialog.value?.showModal();
  } else {
    dialog.value?.close();
    (previousFocus?.isConnected ? previousFocus : trigger.value)?.focus();
  }
}, { flush: 'post' });

watch(status, () => {
  if (dialog.value?.open && !dialog.value.contains(document.activeElement)) {
    dialog.value?.querySelector('button')?.focus();
  }
}, { flush: 'post' });

onMounted(() => { void check(true); });
</script>

<template>
  <button
    ref="trigger"
    type="button"
    class="flex items-center gap-2 rounded-lg bg-slate-800/50 px-3 py-1.5 text-sm text-slate-200 transition-colors hover:bg-slate-700/50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-400"
    @click="check()"
  >
    <i class="fa-solid fa-download" aria-hidden="true"></i>
    {{ status === 'available' ? 'Update available' : status === 'installing' ? 'Updating...' : status === 'installed' ? 'Restart to finish' : 'Check for updates' }}
  </button>

  <Teleport to="body">
    <dialog
      ref="dialog"
      aria-labelledby="app-update-title"
      aria-describedby="app-update-description"
      class="m-auto max-h-[85vh] w-[calc(100%_-_2rem)] max-w-md overflow-y-auto rounded-xl border border-slate-600 bg-slate-800 p-6 text-slate-200 shadow-xl backdrop:bg-black/60 backdrop:backdrop-blur-sm"
      @cancel.prevent="close"
      @close="close"
      @keydown.stop
    >
      <div class="mb-4 flex items-center justify-between gap-4">
        <h2 id="app-update-title" class="text-lg font-semibold text-white">{{ title }}</h2>
        <button
          type="button"
          aria-label="Close update popup"
          class="flex h-8 w-8 items-center justify-center rounded text-slate-300 hover:bg-slate-700 focus-visible:outline-2 focus-visible:outline-indigo-400"
          @click="close"
        >
          <i class="fa-solid fa-xmark" aria-hidden="true"></i>
        </button>
      </div>
      <p class="mb-3 text-sm text-slate-400">Current version: {{ info?.currentVersion ?? appVersion }}</p>
      <div id="app-update-description" class="space-y-3 text-sm" role="status" aria-live="polite" aria-atomic="true">
        <p v-if="status === 'checking'">
          <i class="fa-solid fa-spinner mr-2 animate-spin motion-reduce:animate-none" aria-hidden="true"></i>
          Checking WinGet for updates...
        </p>
        <template v-else-if="status === 'available'">
          <p>Version <strong class="text-white">{{ info?.availableVersion }}</strong> is available through WinGet.</p>
          <p>Save your queries before updating. The Windows installer may ask to close the app or approve administrator access.</p>
        </template>
        <p v-else-if="status === 'current'">You are up to date. WinGet has no newer version for this app.</p>
        <p v-else-if="status === 'not-installed'">WinGet could not find an installed copy of this app. Install SQL Plan For Dummies with WinGet or the MSI installer, then check again.</p>
        <p v-else-if="status === 'installing'">
          <i class="fa-solid fa-spinner mr-2 animate-spin motion-reduce:animate-none" aria-hidden="true"></i>
          WinGet is updating the app. Follow any Windows installer prompts. You can close this popup while the update continues.
        </p>
        <p v-else-if="status === 'installed'">The update finished. Save any open queries, then restart the app to use the new version.</p>
        <p v-else-if="status === 'error'">The update could not be completed. Check the details below and try again.</p>
      </div>
      <p v-if="error" role="alert" class="mt-3 max-h-48 overflow-y-auto whitespace-pre-wrap break-words rounded-lg border border-red-700/50 bg-red-900/20 p-3 text-sm text-red-200">{{ error }}</p>
      <div class="mt-6 flex justify-end gap-3">
        <button
          type="button"
          class="rounded-lg border border-slate-600 px-4 py-2 text-sm text-slate-200 hover:bg-slate-700 focus-visible:outline-2 focus-visible:outline-indigo-400"
          @click="close"
        >{{ status === 'available' || status === 'installed' ? 'Later' : 'Close' }}</button>
        <button
          v-if="status === 'available'"
          type="button"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-400"
          @click="install"
        >Update with WinGet</button>
        <button
          v-else-if="status === 'installed'"
          type="button"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-400"
          @click="restart"
        >Restart app</button>
        <button
          v-else-if="status === 'error' || status === 'not-installed' || status === 'current'"
          type="button"
          class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-400"
          @click="check()"
        >Check again</button>
      </div>
    </dialog>
  </Teleport>
</template>
