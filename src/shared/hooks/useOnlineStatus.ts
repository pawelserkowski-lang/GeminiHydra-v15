// src/shared/hooks/useOnlineStatus.ts
/**
 * GeminiHydra v15 - Online Status Hook (#25)
 * ============================================
 * Detects navigator.onLine changes and exposes a boolean reactive state.
 * Used by OfflineBanner to show/hide connectivity warnings.
 */

import { useSyncExternalStore } from 'react';

function subscribe(onStoreChange: () => void): () => void {
  window.addEventListener('online', onStoreChange);
  window.addEventListener('offline', onStoreChange);
  return () => {
    window.removeEventListener('online', onStoreChange);
    window.removeEventListener('offline', onStoreChange);
  };
}

function getSnapshot(): boolean {
  return navigator.onLine;
}

function getServerSnapshot(): boolean {
  return true;
}

/**
 * Returns `true` when the browser is online, `false` when offline.
 * Reacts to `online`/`offline` window events.
 */
export function useOnlineStatus(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}

export default useOnlineStatus;
