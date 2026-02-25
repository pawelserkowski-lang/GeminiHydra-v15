// src/stores/viewStore.ts
/**
 * GeminiHydra v15 - Zustand View Store
 * =====================================
 * Manages SPA navigation (no router), session/tab state, chat history.
 * Refactored to use the Slice Pattern for better maintainability.
 */

import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import { type ChatSlice, createChatSlice } from './slices/chatSlice';
import { createSessionSlice, type SessionSlice } from './slices/sessionSlice';
import { createViewSlice, type ViewSlice } from './slices/viewSlice';
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
          chatHistory: state.chatHistory,
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
