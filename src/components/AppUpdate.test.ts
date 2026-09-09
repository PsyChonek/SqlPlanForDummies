import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import AppUpdate from './AppUpdate.vue';
import { tauriInvoke } from '../composables/tauriApi';

vi.mock('../composables/tauriApi', () => ({ tauriInvoke: vi.fn() }));
const invoke = vi.mocked(tauriInvoke);
enableAutoUnmount(afterEach);

describe('AppUpdate popup', () => {
  beforeEach(() => { invoke.mockReset(); });

  it('shows the available version, dismisses with Escape, and restores focus', async () => {
    invoke.mockResolvedValue({ currentVersion: '2.7.0', availableVersion: '2.8.0', installed: true });
    const wrapper = mount(AppUpdate, { attachTo: document.body });
    await flushPromises();
    const dialog = document.querySelector('dialog')!;
    expect(dialog.open).toBe(true);
    expect(dialog.textContent).toContain('2.8.0');
    expect(dialog.getAttribute('aria-labelledby')).toBe('app-update-title');
    dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
    await flushPromises();
    expect(dialog.open).toBe(false);
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(dialog.open).toBe(true);
    const later = Array.from(dialog.querySelectorAll('button')).find(button => button.textContent === 'Later')!;
    later.click();
    await flushPromises();
    expect(dialog.open).toBe(false);
    expect(document.activeElement).toBe(wrapper.get('button').element);
    expect(invoke.mock.calls.every(([command]) => command === 'check_app_update')).toBe(true);
    wrapper.unmount();
  });

  it('starts installation only after clicking Update and displays restart after success', async () => {
    invoke.mockResolvedValueOnce({ currentVersion: '2.7.0', availableVersion: '2.8.0', installed: true });
    const wrapper = mount(AppUpdate, { attachTo: document.body });
    await flushPromises();
    const dialog = document.querySelector('dialog')!;
    invoke.mockResolvedValueOnce(undefined);
    const install = Array.from(dialog.querySelectorAll('button')).find(button => button.textContent === 'Update with WinGet')!;
    install.click();
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith('install_app_update');
    expect(dialog.textContent).toContain('Update installed');
    expect(dialog.textContent).toContain('Restart app');
    wrapper.unmount();
  });
});
