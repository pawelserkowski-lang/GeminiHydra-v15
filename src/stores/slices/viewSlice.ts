import { StateCreator } from 'zustand';
import { View } from '../types';
import { ViewStoreState } from '../viewStore';

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

export const createViewSlice: StateCreator<
  ViewStoreState,
  [],
  [],
  ViewSlice
> = (set) => ({
  currentView: 'home',
  sidebarCollapsed: false,
  activeModel: null,

  setCurrentView: (view) => set({ currentView: view }),
  setActiveModel: (model) => set({ activeModel: model }),

  setSidebarCollapsed: (collapsed) => {
    try {
      localStorage.setItem('geminihydra_sidebar_collapsed', String(collapsed));
    } catch {
      /* ignore */
    }
    set({ sidebarCollapsed: collapsed });
  },

  toggleSidebar: () =>
    set((state) => {
      const next = !state.sidebarCollapsed;
      try {
        localStorage.setItem('geminihydra_sidebar_collapsed', String(next));
      } catch {
        /* ignore */
      }
      return { sidebarCollapsed: next };
    }),
});
