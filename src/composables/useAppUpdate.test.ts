import { useAppUpdate } from './useAppUpdate';
import { tauriInvoke } from './tauriApi';

vi.mock('./tauriApi', () => ({ tauriInvoke: vi.fn() }));
const invoke = vi.mocked(tauriInvoke);
const available = { currentVersion: '2.7.0', availableVersion: '2.8.0', installed: true };

describe('useAppUpdate', () => {
  beforeEach(() => { invoke.mockReset(); });

  it('offers a startup update without starting installation', async () => {
    invoke.mockResolvedValue(available);
    const update = useAppUpdate();
    await update.check(true);
    expect(update.status.value).toBe('available');
    expect(update.isOpen.value).toBe(true);
    expect(invoke.mock.calls).toEqual([['check_app_update']]);
    update.close();
    expect(update.isOpen.value).toBe(false);
  });

  it.each([
    [{ ...available, availableVersion: null }, 'current'],
    [{ ...available, availableVersion: null, installed: false }, 'not-installed'],
  ])('keeps startup quiet for %s and shows the result on manual check', async (info, status) => {
    invoke.mockResolvedValue(info);
    const update = useAppUpdate();
    await update.check(true);
    expect(update.status.value).toBe(status);
    expect(update.isOpen.value).toBe(false);
    await update.check();
    expect(update.isOpen.value).toBe(true);
  });

  it('keeps startup failures quiet and allows a successful retry', async () => {
    invoke.mockRejectedValueOnce(new Error('WinGet is unavailable'));
    const update = useAppUpdate();
    await update.check(true);
    expect(update.isOpen.value).toBe(false);
    expect(update.error.value).toBe('WinGet is unavailable');
    invoke.mockResolvedValueOnce(available);
    await update.check();
    expect(update.status.value).toBe('available');
    expect(update.error.value).toBe('');
  });

  it('does not reopen a dismissed popup when a pending startup check finishes', async () => {
    let resolveCheck!: (value: typeof available) => void;
    invoke.mockReturnValueOnce(new Promise(resolve => { resolveCheck = resolve; }));
    const update = useAppUpdate();
    const pending = update.check(true);
    await update.check();
    update.close();
    resolveCheck(available);
    await pending;
    expect(update.status.value).toBe('available');
    expect(update.isOpen.value).toBe(false);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('prevents duplicate installs and retains progress when the popup is reopened', async () => {
    invoke.mockResolvedValueOnce(available);
    const update = useAppUpdate();
    await update.check();
    let finishInstall!: () => void;
    invoke.mockReturnValueOnce(new Promise<void>(resolve => { finishInstall = resolve; }));
    const pending = update.install();
    await update.install();
    update.close();
    await update.check();
    expect(update.isOpen.value).toBe(true);
    expect(update.status.value).toBe('installing');
    expect(invoke).toHaveBeenCalledTimes(2);
    finishInstall();
    await pending;
    expect(update.status.value).toBe('installed');
    update.close();
    await update.check();
    expect(update.status.value).toBe('installed');
    invoke.mockResolvedValueOnce(undefined);
    await update.restart();
    expect(invoke).toHaveBeenLastCalledWith('restart_after_update');
  });

  it('shows installer errors and never offers a restart after failure', async () => {
    invoke.mockResolvedValueOnce(available).mockRejectedValueOnce('Installer cancelled');
    const update = useAppUpdate();
    await update.check();
    await update.install();
    expect(update.status.value).toBe('error');
    expect(update.error.value).toBe('Installer cancelled');
    await update.restart();
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it('cannot install without an available update', async () => {
    const update = useAppUpdate();
    await update.install();
    expect(invoke).not.toHaveBeenCalled();
  });
});
