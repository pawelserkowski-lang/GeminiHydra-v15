import { StateCreator } from 'zustand';
import { ChatTab, Message } from '../types';
import { ViewStoreState } from '../viewStore';
import { MAX_MESSAGES_PER_SESSION, MAX_TITLE_LENGTH, sanitizeContent, sanitizeTitle } from '../utils';

export interface ChatSlice {
  chatHistory: Record<string, Message[]>;
  tabs: ChatTab[];
  activeTabId: string | null;

  openTab: (sessionId: string) => void;
  closeTab: (tabId: string) => void;
  switchTab: (tabId: string) => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
  togglePinTab: (tabId: string) => void;

  addMessage: (msg: Message) => void;
  updateLastMessage: (content: string) => void;
  clearHistory: () => void;
}

export const createChatSlice: StateCreator<
  ViewStoreState,
  [],
  [],
  ChatSlice
> = (set) => ({
  chatHistory: {},
  tabs: [],
  activeTabId: null,

  openTab: (sessionId) =>
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

  closeTab: (tabId) =>
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

  switchTab: (tabId) =>
    set((state) => {
      const tab = state.tabs.find((t) => t.id === tabId);
      if (!tab) return state;
      return {
        activeTabId: tabId,
        currentSessionId: tab.sessionId,
        currentView: 'chat',
      };
    }),

  reorderTabs: (fromIndex, toIndex) =>
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

  togglePinTab: (tabId) =>
    set((state) => ({
      tabs: state.tabs.map((t) => (t.id === tabId ? { ...t, isPinned: !t.isPinned } : t)),
    })),

  addMessage: (msg) =>
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

  updateLastMessage: (content) =>
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
});
