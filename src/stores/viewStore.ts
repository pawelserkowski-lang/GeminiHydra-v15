// src/stores/viewStore.ts
/**
 * GeminiHydra v15 - Zustand View Store
 * =====================================
 * Manages SPA navigation (no router), session/tab state, chat history.
 * Refactored to use the Slice Pattern for better maintainability.
 */

import { useCallback } from 'react';
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import { useShallow } from 'zustand/react/shallow';
import { type ChatSlice, createChatSlice } from './slices/chatSlice';
import { createSessionSlice, type SessionSlice } from './slices/sessionSlice';
import { createViewSlice, type ViewSlice } from './slices/viewSlice';
import type { Message, Session } from './types';
// ============================================================================
// TYPES
// ============================================================================

export type ViewStoreState = ViewSlice & SessionSlice & ChatSlice;

// Re-export types for backward compatibility
export * from './types';
export * from './utils'; // Optional, if consumers need constants

// ============================================================================
// STORE
// ============================================================================

export const useViewStore = create<ViewStoreState>()(
  devtools(
    persist(
      (...a) => ({
        ...createViewSlice(...a),
        ...createSessionSlice(...a),
        ...createChatSlice(...a),
      }),
      {
        name: 'geminihydra-v15-state',
        partialize: (state) => ({
          currentView: state.currentView,
          sidebarCollapsed: state.sidebarCollapsed,
          sessions: state.sessions,
          currentSessionId: state.currentSessionId,
          chatHistory: Object.fromEntries(
            Object.entries(state.chatHistory)
              .sort(([, a], [, b]) => {
                const lastA = a[a.length - 1]?.timestamp ?? 0;
                const lastB = b[b.length - 1]?.timestamp ?? 0;
                return lastB - lastA;
              })
              .slice(0, 20),
          ),
          tabs: state.tabs,
          activeTabId: state.activeTabId,
        }),
        merge: (persisted, current) => {
          const p = persisted as Partial<ViewStoreState>;
          const merged = { ...current, ...p };
          // Always start from home view on app launch
          merged.currentView = 'home';
          // Clear tabs on launch (sessions/history preserved)
          merged.tabs = [];
          merged.activeTabId = null;
          return merged;
        },
      },
    ),
    { name: 'GeminiHydra/ViewStore', enabled: import.meta.env.DEV },
  ),
);

// ============================================================================
// MEMOIZED SELECTORS (#31)
// ============================================================================
// These selectors prevent unnecessary re-renders when unrelated store state
// changes. Components that only need the session ID won't re-render when
// chatHistory changes, etc.

/** Returns just the current session ID string (or null). Cheapest selector. */
export function useCurrentSessionId(): string | null {
  return useViewStore((s) => s.currentSessionId);
}

/** Returns the current Session object (or undefined). Uses useShallow for stable reference. */
export function useCurrentSession(): Session | undefined {
  return useViewStore(
    useCallback((s: ViewStoreState) => s.sessions.find((sess) => sess.id === s.currentSessionId), []),
  );
}

/** Returns the messages array for the current session. Uses useShallow for stable array reference. */
export function useCurrentChatHistory(): Message[] {
  return useViewStore(useShallow((s) => (s.currentSessionId ? (s.chatHistory[s.currentSessionId] ?? []) : [])));
}
