import type { StateCreator } from 'zustand';
import type { View } from '../types';
import type { ViewStoreState } from '../viewStore';

export interface ViewSlice {
  currentView: View;
  sidebarCollapsed: boolean;
  /** Model ID reported by the last WS Start message (e.g. "gemini-3.1-pro-preview") */
  activeModel: string | null;
  setCurrentView: (view: View) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
  setActiveModel: (model: string) => void;
}

const SIDEBAR_STORAGE_KEY = 'geminihydra_sidebar_collapsed';

function persistSidebarState(collapsed: boolean): void {
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, String(collapsed));
  } catch {
    /* ignore */
  }
}

export const createViewSlice: StateCreator<ViewStoreState, [], [], ViewSlice> = (set) => ({
  currentView: 'home',
  sidebarCollapsed: false,
  activeModel: null,

  setCurrentView: (view) => set({ currentView: view }),
  setActiveModel: (model) => set({ activeModel: model }),

  setSidebarCollapsed: (collapsed) => {
    persistSidebarState(collapsed);
    set({ sidebarCollapsed: collapsed });
  },

  toggleSidebar: () =>
    set((state) => {
      const next = !state.sidebarCollapsed;
      persistSidebarState(next);
      return { sidebarCollapsed: next };
    }),
});
