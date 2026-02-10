/**
 * Safe wrapper around Tauri's invoke API.
 * Falls back to an error when running outside Tauri (e.g. npm run dev in browser).
 */
import { invoke } from '@tauri-apps/api/core';

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error('Tauri backend not available. Run with "npm run tauri dev" to use database features.');
  }
  return invoke<T>(cmd, args);
}
