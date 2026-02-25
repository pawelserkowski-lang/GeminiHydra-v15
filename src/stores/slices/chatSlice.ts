import type { StateCreator } from 'zustand';
import type { ChatTab, Message } from '../types';
import { MAX_MESSAGES_PER_SESSION, MAX_TITLE_LENGTH, sanitizeContent, sanitizeTitle } from '../utils';
import type { ViewStoreState } from '../viewStore';

// ── Helpers ─────────────────────────────────────────────────────────────────

const MAX_CONTENT_LENGTH = 100_000;

/** Append a message to a session's history, enforcing the max messages limit. */
function appendMessage(history: Record<string, Message[]>, sessionId: string, msg: Message): Message[] {
  const current = history[sessionId] || [];
  const sanitizedMsg: Message = { ...msg, content: sanitizeContent(msg.content, MAX_CONTENT_LENGTH) };
  let updated = [...current, sanitizedMsg];
  if (updated.length > MAX_MESSAGES_PER_SESSION) {
    updated = updated.slice(-MAX_MESSAGES_PER_SESSION);
  }
  return updated;
}

/** Generate an auto-title from the first user message in a session. */
function autoTitle(
  msg: Message,
  existingMessages: Message[],
  sessionId: string,
  sessions: ViewStoreState['sessions'],
  tabs: ChatTab[],
): { sessions: ViewStoreState['sessions']; tabs: ChatTab[] } {
  if (msg.role !== 'user' || existingMessages.length > 0) {
    return { sessions, tabs };
  }
  const title = sanitizeTitle(msg.content.substring(0, 30) + (msg.content.length > 30 ? '...' : ''), MAX_TITLE_LENGTH);
  return {
    sessions: sessions.map((s) => (s.id === sessionId ? { ...s, title } : s)),
    tabs: tabs.map((t) => (t.sessionId === sessionId ? { ...t, title } : t)),
  };
}

/** Append content to the last message of a session. */
function appendToLastMessage(
  history: Record<string, Message[]>,
  sessionId: string,
  content: string,
): Record<string, Message[]> | null {
  const messages = history[sessionId] || [];
  if (messages.length === 0) return null;
  const lastMsg = messages[messages.length - 1];
  if (!lastMsg) return null;
  const newMessages = [...messages];
  newMessages[newMessages.length - 1] = {
    ...lastMsg,
    content: sanitizeContent(lastMsg.content + content, MAX_CONTENT_LENGTH),
  };
  return { ...history, [sessionId]: newMessages };
}

// ── Interface ───────────────────────────────────────────────────────────────

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

  /** Add a message to a specific session (for background streaming). */
  addMessageToSession: (sessionId: string, msg: Message) => void;
  /** Append content to the last message of a specific session. */
  updateLastMessageInSession: (sessionId: string, content: string) => void;
}

// ── Slice ───────────────────────────────────────────────────────────────────

export const createChatSlice: StateCreator<ViewStoreState, [], [], ChatSlice> = (set) => ({
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
      const sid = state.currentSessionId;
      if (!sid) return state;
      const currentMessages = state.chatHistory[sid] || [];
      const updatedMessages = appendMessage(state.chatHistory, sid, msg);
      const { sessions, tabs } = autoTitle(msg, currentMessages, sid, state.sessions, state.tabs);
      return {
        chatHistory: { ...state.chatHistory, [sid]: updatedMessages },
        sessions,
        tabs,
      };
    }),

  updateLastMessage: (content) =>
    set((state) => {
      if (!state.currentSessionId) return state;
      const updated = appendToLastMessage(state.chatHistory, state.currentSessionId, content);
      return updated ? { chatHistory: updated } : state;
    }),

  addMessageToSession: (sessionId, msg) =>
    set((state) => {
      const currentMessages = state.chatHistory[sessionId] || [];
      const updatedMessages = appendMessage(state.chatHistory, sessionId, msg);
      const { sessions, tabs } = autoTitle(msg, currentMessages, sessionId, state.sessions, state.tabs);
      return {
        chatHistory: { ...state.chatHistory, [sessionId]: updatedMessages },
        sessions,
        tabs,
      };
    }),

  updateLastMessageInSession: (sessionId, content) =>
    set((state) => {
      const updated = appendToLastMessage(state.chatHistory, sessionId, content);
      return updated ? { chatHistory: updated } : state;
    }),
});
