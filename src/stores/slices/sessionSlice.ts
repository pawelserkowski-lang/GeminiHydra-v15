import type { StateCreator } from 'zustand';
import type { Session } from '../types';
import { MAX_SESSIONS, MAX_TITLE_LENGTH, sanitizeTitle } from '../utils';
import type { ViewStoreState } from '../viewStore';

export interface SessionSlice {
  sessions: Session[];
  currentSessionId: string | null;
  createSession: () => void;
  createSessionWithId: (id: string, title: string) => void;
  deleteSession: (id: string) => void;
  selectSession: (id: string) => void;
  updateSessionTitle: (id: string, title: string) => void;
  hydrateSessions: (sessions: Session[]) => void;
}

// Helper to prevent duplication in createSession and createSessionWithId
const addSessionToState = (state: ViewStoreState, newSession: Session) => {
  let sessions = [newSession, ...state.sessions];
  const changes: Partial<ViewStoreState> = {
    currentSessionId: newSession.id,
  };

  if (sessions.length > MAX_SESSIONS) {
    const removedIds = sessions.slice(MAX_SESSIONS).map((s) => s.id);
    sessions = sessions.slice(0, MAX_SESSIONS);

    const newHistory = { ...state.chatHistory };
    for (const removedId of removedIds) {
      delete newHistory[removedId];
    }
    changes.chatHistory = { ...newHistory, [newSession.id]: [] };
  } else {
    changes.chatHistory = { ...state.chatHistory, [newSession.id]: [] };
  }

  changes.sessions = sessions;
  return changes;
};

export const createSessionSlice: StateCreator<ViewStoreState, [], [], SessionSlice> = (set) => ({
  sessions: [],
  currentSessionId: null,

  createSession: () => {
    const id = crypto.randomUUID();
    const newSession: Session = {
      id,
      title: 'New Chat',
      createdAt: Date.now(),
    };

    set((state) => addSessionToState(state, newSession));
  },

  createSessionWithId: (id, title) => {
    const newSession: Session = {
      id,
      title,
      createdAt: Date.now(),
    };

    set((state) => {
      if (state.sessions.some((s) => s.id === id)) {
        return { currentSessionId: id };
      }
      return addSessionToState(state, newSession);
    });
  },

  deleteSession: (id) =>
    set((state) => {
      const newSessions = state.sessions.filter((s) => s.id !== id);
      const newHistory = { ...state.chatHistory };
      delete newHistory[id];

      let newCurrentId = state.currentSessionId;
      if (state.currentSessionId === id) {
        newCurrentId = newSessions.length > 0 ? (newSessions[0]?.id ?? null) : null;
      }

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

  selectSession: (id) =>
    set((state) => {
      const exists = state.sessions.some((s) => s.id === id);
      if (!exists) return state;
      return { currentSessionId: id };
    }),

  updateSessionTitle: (id, title) =>
    set((state) => {
      const sanitized = sanitizeTitle(title, MAX_TITLE_LENGTH);
      return {
        sessions: state.sessions.map((s) => (s.id === id ? { ...s, title: sanitized } : s)),
        tabs: state.tabs.map((t) => (t.sessionId === id ? { ...t, title: sanitized } : t)),
      };
    }),

  hydrateSessions: (dbSessions) =>
    set((state) => {
      const dbIds = new Set(dbSessions.map((s) => s.id));
      const merged = [...dbSessions, ...state.sessions.filter((s) => !dbIds.has(s.id))];

      const mergedIds = new Set(merged.map((s) => s.id));
      const currentSessionId =
        state.currentSessionId && mergedIds.has(state.currentSessionId)
          ? state.currentSessionId
          : merged.length > 0
            ? (merged[0]?.id ?? null)
            : null;

      return {
        sessions: merged,
        currentSessionId,
      };
    }),
});
