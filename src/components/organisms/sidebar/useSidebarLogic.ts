// src/components/organisms/sidebar/useSidebarLogic.ts
import { useCallback, useMemo, useState } from 'react';
import { useSessionSync } from '@/features/chat/hooks/useSessionSync';
import type { View } from '@/stores/types';
import { useViewStore } from '@/stores/viewStore';

export function useSidebarLogic() {
  // Store selectors (individual for minimal re-renders)
  const currentView = useViewStore((s) => s.currentView);
  const setCurrentView = useViewStore((s) => s.setCurrentView);
  const chatHistory = useViewStore((s) => s.chatHistory);
  const sidebarCollapsed = useViewStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useViewStore((s) => s.toggleSidebar);

  // Session sync (DB + localStorage)
  const {
    sessions,
    currentSessionId,
    selectSession,
    createSessionWithSync,
    deleteSessionWithSync,
    renameSessionWithSync,
    unlockSessionWithSync,
  } = useSessionSync();

  // Session search/filter (#19)
  const [sessionSearchQuery, setSessionSearchQuery] = useState('');
  const handleSessionSearch = useCallback((query: string) => {
    setSessionSearchQuery(query);
  }, []);

  // Sessions sorted by creation date (newest first), then filtered by search
  const sortedSessions = useMemo(() => {
    const sorted = [...sessions].sort((a, b) => b.createdAt - a.createdAt);
    if (!sessionSearchQuery) return sorted;
    return sorted.filter((s) => s.title.toLowerCase().includes(sessionSearchQuery));
  }, [sessions, sessionSearchQuery]);

  // Collapsible sessions toggle
  const [showSessions, setShowSessions] = useState(true);

  // #42 — Keyboard navigation for session list
  const [focusedSessionIndex, setFocusedSessionIndex] = useState(-1);

  const handleSessionListKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setFocusedSessionIndex((i) => (i + 1) % sortedSessions.length);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setFocusedSessionIndex((i) => (i - 1 + sortedSessions.length) % sortedSessions.length);
      } else if (e.key === 'Enter' && focusedSessionIndex >= 0 && sortedSessions[focusedSessionIndex]) {
        e.preventDefault();
        selectSession(sortedSessions[focusedSessionIndex].id);
        setCurrentView('chat');
      }
    },
    [sortedSessions, focusedSessionIndex, selectSession, setCurrentView],
  );

  // Session CRUD handlers
  const handleSelectSession = useCallback(
    (id: string) => {
      selectSession(id);
      setCurrentView('chat');
    },
    [selectSession, setCurrentView],
  );

  const handleNewChat = useCallback(() => {
    void createSessionWithSync();
    setCurrentView('chat');
  }, [createSessionWithSync, setCurrentView]);

  const handleDeleteSession = useCallback(
    (id: string) => {
      void deleteSessionWithSync(id);
    },
    [deleteSessionWithSync],
  );

  const handleRenameSession = useCallback(
    (id: string, newTitle: string) => {
      void renameSessionWithSync(id, newTitle);
    },
    [renameSessionWithSync],
  );

  const handleUnlockSession = useCallback(
    (id: string) => {
      void unlockSessionWithSync(id);
    },
    [unlockSessionWithSync],
  );

  const handleNavClick = useCallback(
    (view: View) => {
      setCurrentView(view);
    },
    [setCurrentView],
  );

  return {
    // Store state
    currentView,
    setCurrentView,
    chatHistory,
    sidebarCollapsed,
    toggleSidebar,

    // Session state
    sessions,
    currentSessionId,
    sortedSessions,
    sessionSearchQuery,
    focusedSessionIndex,
    showSessions,
    setShowSessions,

    // Session handlers
    handleSessionSearch,
    handleSessionListKeyDown,
    handleSelectSession,
    handleNewChat,
    handleDeleteSession,
    handleRenameSession,
    handleUnlockSession,
    handleNavClick,
  };
}
