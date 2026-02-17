import { StateCreator } from 'zustand';
import { View } from '../types';
import { ViewStoreState } from '../viewStore';

export interface ViewSlice {
  currentView: View;
  sidebarCollapsed: boolean;
  setCurrentView: (view: View) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
}

export const createViewSlice: StateCreator<
  ViewStoreState,
  [],
  [],
  ViewSlice
> = (set) => ({
  currentView: 'home',
  sidebarCollapsed: false,

  setCurrentView: (view) => set({ currentView: view }),

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
