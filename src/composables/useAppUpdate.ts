import { ref } from 'vue';
import { tauriInvoke } from './tauriApi';

interface UpdateInfo {
  currentVersion: string;
  availableVersion: string | null;
  installed: boolean;
}

type UpdateStatus = 'idle' | 'checking' | 'available' | 'current' | 'not-installed' | 'installing' | 'installed' | 'error';

export function useAppUpdate() {
  const status = ref<UpdateStatus>('idle');
  const info = ref<UpdateInfo | null>(null);
  const error = ref('');
  const isOpen = ref(false);
  let dismissed = false;

  function close() {
    isOpen.value = false;
    dismissed = true;
  }

  async function check(automatic = false) {
    if (!automatic) isOpen.value = true;
    if (['checking', 'installing', 'installed'].includes(status.value)) return;
    status.value = 'checking';
    error.value = '';
    info.value = null;
    try {
      info.value = await tauriInvoke<UpdateInfo>('check_app_update');
      status.value = !info.value.installed ? 'not-installed'
        : info.value.availableVersion ? 'available' : 'current';
      if (automatic && status.value === 'available' && !dismissed) isOpen.value = true;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      status.value = 'error';
    }
  }

  async function install() {
    if (status.value !== 'available') return;
    status.value = 'installing';
    error.value = '';
    try {
      await tauriInvoke<void>('install_app_update');
      status.value = 'installed';
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      status.value = 'error';
    }
  }

  async function restart() {
    if (status.value !== 'installed') return;
    error.value = '';
    try {
      await tauriInvoke<void>('restart_after_update');
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    }
  }

  return { status, info, error, isOpen, check, install, restart, close };
}
