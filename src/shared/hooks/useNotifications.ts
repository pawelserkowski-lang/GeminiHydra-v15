// src/shared/hooks/useNotifications.ts
/** Jaskier Shared Pattern */
/**
 * Notification Center Store
 * =========================
 * Zustand store for managing in-app notifications.
 * Replaces toast spam with a structured notification list.
 * Keeps last 50 notifications, supports dismiss/clear.
 */

import { create } from 'zustand';

interface Notification {
  id: string;
  type: 'error' | 'warning' | 'success' | 'info';
  message: string;
  timestamp: number;
  dismissed: boolean;
}

interface NotificationStore {
  notifications: Notification[];
  add: (type: Notification['type'], message: string) => void;
  dismiss: (id: string) => void;
  dismissAll: () => void;
  clear: () => void;
}

export const useNotifications = create<NotificationStore>((set) => ({
  notifications: [],
  add: (type, message) =>
    set((state) => ({
      notifications: [
        ...state.notifications,
        {
          id: crypto.randomUUID(),
          type,
          message,
          timestamp: Date.now(),
          dismissed: false,
        },
      ].slice(-50), // Keep last 50
    })),
  dismiss: (id) =>
    set((state) => ({
      notifications: state.notifications.map((n) => (n.id === id ? { ...n, dismissed: true } : n)),
    })),
  dismissAll: () =>
    set((state) => ({
      notifications: state.notifications.map((n) => ({ ...n, dismissed: true })),
    })),
  clear: () => set({ notifications: [] }),
}));
