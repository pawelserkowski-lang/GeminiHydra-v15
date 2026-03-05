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
  
  // Auto-compaction triggers at >25 messages
  if (updated.length > 25) {
    const cutIndex = updated.length - 15;
    const messagesToCompact = updated.slice(0, cutIndex).filter(m => m.role !== 'system');
    
    // If we haven't already started compacting these messages
    if (messagesToCompact.length > 0 && !updated[0].content.includes('Compacting history')) {
      const compactedMessageId = crypto.randomUUID();
      const compactedMessage: Message = {
        id: compactedMessageId,
        role: 'system',
        content: '_[System] Compacting history to save tokens..._',
        timestamp: Date.now()
      };
      
      // Keep the new system message + the latest 15 messages
      updated = [compactedMessage, ...updated.slice(cutIndex)];
      
      // Fire off background summarization (fire-and-forget)
      const summaryContext = messagesToCompact.map(m => $($m.role.toUpperCase()): $($m.content)).join('\n\n');
      
      // Use standard JS fetch to call our API to summarize without blocking the UI
      fetch('/api/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          messages: [
            { role: 'system', content: 'You are a summarization assistant. Summarize the following chat history into a dense, token-efficient paragraph of facts and context. Do not use conversational filler.' },
            { role: 'user', content: summaryContext }
          ],
          model: 'gemini-2.5-flash',
          temperature: 0.1,
          max_tokens: 500,
          stream: false
        })
      })
      .then(res => res.json())
      .then(data => {
        if (data.result) {
          const summary = data.result;
          const store = useViewStore.getState();
          const currentSessionMsgs = store.chatHistory[sessionId] ?? [];
          const newMsgs = currentSessionMsgs.map(m => 
            m.id === compactedMessageId 
              ? { ...m, content: **[System: Compacted History]**\n\n$($summary) }
              : m
          );
          store.chatHistory[sessionId] = newMsgs;
          useViewStore.setState({ chatHistory: { ...store.chatHistory } });
        }
      })
      .catch(err => {
        console.error('Failed to compact history:', err);
        const store = useViewStore.getState();
        const currentSessionMsgs = store.chatHistory[sessionId] ?? [];
        const newMsgs = currentSessionMsgs.map(m => 
          m.id === compactedMessageId 
            ? { ...m, content: '_[System] History automatically compacted to save tokens. Older messages archived._' }
            : m
        );
        store.chatHistory[sessionId] = newMsgs;
        useViewStore.setState({ chatHistory: { ...store.chatHistory } });
      });
    }
  } else if (updated.length > MAX_MESSAGES_PER_SESSION) {
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
  /** Add or update agent/tool execution messages in the chat stream. */
  appendAgentToolAction: (sessionId: string, agentId: string, content: string) => void;
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

  appendAgentToolAction: (sessionId, agentId, content) =>
    set((state) => {
      const messages = state.chatHistory[sessionId] || [];
      if (messages.length === 0) return state; // Only append if there's an active conversation

      const lastMsg = messages[messages.length - 1];
      if (!lastMsg) return state;

      const newMessages = [...messages];
      newMessages[newMessages.length - 1] = {
        ...lastMsg,
        role: lastMsg.role,
        content: sanitizeContent(
          lastMsg.content + `\n\n> **[Agent: ${agentId}]** Tool action executed:\n> ${content}\n`,
          MAX_CONTENT_LENGTH,
        ),
      } as Message;

      return { chatHistory: { ...state.chatHistory, [sessionId]: newMessages } };
    }),
});


