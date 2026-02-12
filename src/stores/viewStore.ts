// src/stores/viewStore.ts
/**
 * GeminiHydra v15 - Zustand View Store
 * =====================================
 * Manages SPA navigation (no router), session/tab state, chat history.
 * Ported from legacy useAppStore with view/session/tab slices.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

// ============================================================================
// TYPES
// ============================================================================

export type View = 'home' | 'chat' | 'agents' | 'history' | 'settings' | 'status';

export interface Session {
  id: string;
  title: string;
  createdAt: number;
}

export interface ChatTab {
  id: string;
  sessionId: string;
  title: string;
  isPinned: boolean;
}

export type MessageRole = 'user' | 'assistant' | 'system';

export interface Message {
  role: MessageRole;
  content: string;
  timestamp: number;
  model?: string;
}

// ============================================================================
// STORE STATE + ACTIONS
// ============================================================================

interface ViewStoreState {
  // UI State
  currentView: View;
  sidebarCollapsed: boolean;

  // Session Management
  sessions: Session[];
  currentSessionId: string | null;
  chatHistory: Record<string, Message[]>;

  // Tab Management
  tabs: ChatTab[];
  activeTabId: string | null;

  // Actions - View
  setCurrentView: (view: View) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;

  // Actions - Sessions
  createSession: () => void;
  deleteSession: (id: string) => void;
  selectSession: (id: string) => void;
  updateSessionTitle: (id: string, title: string) => void;

  // Actions - Tabs
  openTab: (sessionId: string) => void;
  closeTab: (tabId: string) => void;
  switchTab: (tabId: string) => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
  togglePinTab: (tabId: string) => void;

  // Actions - Messages
  addMessage: (msg: Message) => void;
  updateLastMessage: (content: string) => void;
  clearHistory: () => void;
}

// ============================================================================
// LIMITS
// ============================================================================

const MAX_SESSIONS = 50;
const MAX_MESSAGES_PER_SESSION = 500;
const MAX_TITLE_LENGTH = 100;

// ============================================================================
// HELPERS
// ============================================================================

function sanitizeTitle(title: string, maxLen: number): string {
  return title.trim().slice(0, maxLen) || 'New Chat';
}

function sanitizeContent(content: string, maxLen: number): string {
  return content.slice(0, maxLen);
}

// ============================================================================
// STORE
// ============================================================================

export const useViewStore = create<ViewStoreState>()(
  persist(
    (set) => ({
      // ========================================
      // Initial State
      // ========================================
      currentView: 'home',
      sidebarCollapsed: false,

      sessions: [],
      currentSessionId: null,
      chatHistory: {},

      tabs: [],
      activeTabId: null,

      // ========================================
      // View Actions
      // ========================================
      setCurrentView: (view: View) => set({ currentView: view }),

      setSidebarCollapsed: (collapsed: boolean) => {
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

      // ========================================
      // Session Actions
      // ========================================
      createSession: () => {
        const id = crypto.randomUUID();
        const newSession: Session = {
          id,
          title: 'New Chat',
          createdAt: Date.now(),
        };

        set((state) => {
          let sessions = [newSession, ...state.sessions];

          if (sessions.length > MAX_SESSIONS) {
            const removedIds = sessions.slice(MAX_SESSIONS).map((s) => s.id);
            sessions = sessions.slice(0, MAX_SESSIONS);

            const newHistory = { ...state.chatHistory };
            for (const removedId of removedIds) {
              delete newHistory[removedId];
            }

            return {
              sessions,
              currentSessionId: id,
              chatHistory: { ...newHistory, [id]: [] },
            };
          }

          return {
            sessions,
            currentSessionId: id,
            chatHistory: { ...state.chatHistory, [id]: [] },
          };
        });
      },

      deleteSession: (id: string) =>
        set((state) => {
          const newSessions = state.sessions.filter((s) => s.id !== id);
          const newHistory = { ...state.chatHistory };
          delete newHistory[id];

          let newCurrentId = state.currentSessionId;
          if (state.currentSessionId === id) {
            newCurrentId = newSessions.length > 0 ? (newSessions[0]?.id ?? null) : null;
          }

          // Also close any tab linked to this session
          const newTabs = state.tabs.filter((t) => t.sessionId !== id);
          let newActiveTabId = state.activeTabId;
          if (state.activeTabId && !newTabs.some((t) => t.id === state.activeTabId)) {
            newActiveTabId = newTabs.length > 0 ? (newTabs[0]?.id ?? null) : null;
          }

          return {
            sessions: newSessions,
            chatHistory: newHistory,
            currentSessionId: newCurrentId,
            tabs: newTabs,
            activeTabId: newActiveTabId,
          };
        }),

      selectSession: (id: string) =>
        set((state) => {
          const exists = state.sessions.some((s) => s.id === id);
          if (!exists) return state;
          return { currentSessionId: id };
        }),

      updateSessionTitle: (id: string, title: string) =>
        set((state) => {
          const sanitized = sanitizeTitle(title, MAX_TITLE_LENGTH);
          return {
            sessions: state.sessions.map((s) => (s.id === id ? { ...s, title: sanitized } : s)),
            tabs: state.tabs.map((t) => (t.sessionId === id ? { ...t, title: sanitized } : t)),
          };
        }),

      // ========================================
      // Tab Actions
      // ========================================
      openTab: (sessionId: string) =>
        set((state) => {
          const existing = state.tabs.find((t) => t.sessionId === sessionId);
          if (existing) {
            return { activeTabId: existing.id, currentSessionId: sessionId };
          }
          const session = state.sessions.find((s) => s.id === sessionId);
          const newTab: ChatTab = {
            id: crypto.randomUUID(),
            sessionId,
            title: session?.title || 'New Chat',
            isPinned: false,
          };
          return {
            tabs: [...state.tabs, newTab],
            activeTabId: newTab.id,
            currentSessionId: sessionId,
          };
        }),

      closeTab: (tabId: string) =>
        set((state) => {
          const tabIndex = state.tabs.findIndex((t) => t.id === tabId);
          if (tabIndex === -1) return state;
          const tab = state.tabs[tabIndex];
          if (tab?.isPinned) return state;

          const newTabs = state.tabs.filter((t) => t.id !== tabId);
          let newActiveTabId = state.activeTabId;
          let newSessionId = state.currentSessionId;

          if (state.activeTabId === tabId) {
            if (newTabs.length > 0) {
              const nextIdx = Math.min(tabIndex, newTabs.length - 1);
              const nextTab = newTabs[nextIdx];
              newActiveTabId = nextTab?.id ?? null;
              newSessionId = nextTab?.sessionId ?? state.currentSessionId;
            } else {
              newActiveTabId = null;
              newSessionId = state.sessions.length > 0 ? (state.sessions[0]?.id ?? null) : null;
            }
          }

          return {
            tabs: newTabs,
            activeTabId: newActiveTabId,
            currentSessionId: newSessionId,
          };
        }),

      switchTab: (tabId: string) =>
        set((state) => {
          const tab = state.tabs.find((t) => t.id === tabId);
          if (!tab) return state;
          return {
            activeTabId: tabId,
            currentSessionId: tab.sessionId,
            currentView: 'chat' as View,
          };
        }),

      reorderTabs: (fromIndex: number, toIndex: number) =>
        set((state) => {
          if (fromIndex < 0 || fromIndex >= state.tabs.length || toIndex < 0 || toIndex >= state.tabs.length) {
            return state;
          }
          const newTabs = [...state.tabs];
          const moved = newTabs.splice(fromIndex, 1)[0];
          if (!moved) return state;
          newTabs.splice(toIndex, 0, moved);
          return { tabs: newTabs };
        }),

      togglePinTab: (tabId: string) =>
        set((state) => ({
          tabs: state.tabs.map((t) => (t.id === tabId ? { ...t, isPinned: !t.isPinned } : t)),
        })),

      // ========================================
      // Message Actions
      // ========================================
      addMessage: (msg: Message) =>
        set((state) => {
          if (!state.currentSessionId) return state;

          const sanitizedMsg: Message = {
            ...msg,
            content: sanitizeContent(msg.content, 100_000),
          };

          const currentMessages = state.chatHistory[state.currentSessionId] || [];

          let updatedMessages = [...currentMessages, sanitizedMsg];
          if (updatedMessages.length > MAX_MESSAGES_PER_SESSION) {
            updatedMessages = updatedMessages.slice(-MAX_MESSAGES_PER_SESSION);
          }

          let updatedSessions = state.sessions;
          let updatedTabs = state.tabs;
          if (msg.role === 'user' && currentMessages.length === 0) {
            const title = sanitizeTitle(
              msg.content.substring(0, 30) + (msg.content.length > 30 ? '...' : ''),
              MAX_TITLE_LENGTH,
            );
            updatedSessions = state.sessions.map((s) => (s.id === state.currentSessionId ? { ...s, title } : s));
            updatedTabs = state.tabs.map((t) => (t.sessionId === state.currentSessionId ? { ...t, title } : t));
          }

          return {
            chatHistory: {
              ...state.chatHistory,
              [state.currentSessionId]: updatedMessages,
            },
            sessions: updatedSessions,
            tabs: updatedTabs,
          };
        }),

      updateLastMessage: (content: string) =>
        set((state) => {
          if (!state.currentSessionId) return state;
          const messages = state.chatHistory[state.currentSessionId] || [];
          if (messages.length === 0) return state;

          const newMessages = [...messages];
          const lastMsg = newMessages[newMessages.length - 1];
          if (!lastMsg) return state;

          newMessages[newMessages.length - 1] = {
            ...lastMsg,
            content: sanitizeContent(lastMsg.content + content, 100_000),
          };

          return {
            chatHistory: {
              ...state.chatHistory,
              [state.currentSessionId]: newMessages,
            },
          };
        }),

      clearHistory: () =>
        set((state) => {
          if (!state.currentSessionId) return state;
          return {
            chatHistory: {
              ...state.chatHistory,
              [state.currentSessionId]: [],
            },
          };
        }),
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
        merged.currentView = 'home' as View;
        // Clear tabs on launch (sessions/history preserved)
        merged.tabs = [];
        merged.activeTabId = null;
        return merged;
      },
    },
  ),
);

// ============================================================================
// SELECTORS
// ============================================================================

export const selectCurrentMessages = (state: ViewStoreState): Message[] => {
  if (!state.currentSessionId) return [];
  return state.chatHistory[state.currentSessionId] || [];
};

export const selectSortedSessions = (state: ViewStoreState): Session[] =>
  [...state.sessions].sort((a, b) => b.createdAt - a.createdAt);

export const selectMessageCount = (state: ViewStoreState, sessionId: string): number =>
  (state.chatHistory[sessionId] || []).length;
