/**
 * useSessionSync — Bridge between viewStore (localStorage) and backend DB.
 *
 * Provides synced CRUD operations that update both the local Zustand store
 * and the Postgres backend. On first load, hydrates localStorage from DB
 * if localStorage is empty.
 */

import { useCallback, useEffect, useRef } from 'react';
import { useViewStore } from '@/stores/viewStore';
import {
  useAddMessageMutation,
  useCreateSessionMutation,
  useDeleteSessionMutation,
  useSessionsQuery,
  useUpdateSessionMutation,
} from './useSessions';
import type { Session } from '@/stores/viewStore';

const MIGRATION_FLAG = 'gh-sessions-migrated-to-db';

export function useSessionSync() {
  const sessions = useViewStore((s) => s.sessions);
  const currentSessionId = useViewStore((s) => s.currentSessionId);
  const createSessionLocal = useViewStore((s) => s.createSession);
  const createSessionWithId = useViewStore((s) => s.createSessionWithId);
  const deleteSessionLocal = useViewStore((s) => s.deleteSession);
  const updateSessionTitleLocal = useViewStore((s) => s.updateSessionTitle);
  const selectSession = useViewStore((s) => s.selectSession);
  const hydrateSessions = useViewStore((s) => s.hydrateSessions);

  const { data: dbSessions, isSuccess: dbLoaded } = useSessionsQuery();

  const createMutation = useCreateSessionMutation();
  const deleteMutation = useDeleteSessionMutation();
  const updateMutation = useUpdateSessionMutation();
  const addMessageMutation = useAddMessageMutation();

  // One-time hydration from DB
  const hydratedRef = useRef(false);
  useEffect(() => {
    if (!dbLoaded || !dbSessions || hydratedRef.current) return;
    hydratedRef.current = true;

    const migrated = localStorage.getItem(MIGRATION_FLAG);

    if (!migrated && sessions.length === 0 && dbSessions.length > 0) {
      // Hydrate from DB into localStorage on first load
      const mapped: Session[] = dbSessions.map((s) => ({
        id: s.id,
        title: s.title,
        createdAt: new Date(s.created_at).getTime(),
      }));
      hydrateSessions(mapped);
      localStorage.setItem(MIGRATION_FLAG, 'true');
    } else if (!migrated) {
      localStorage.setItem(MIGRATION_FLAG, 'true');
    }
  }, [dbLoaded, dbSessions, sessions.length, hydrateSessions]);

  /** Create a session in DB, then add to viewStore with the DB-generated UUID. */
  const createSessionWithSync = useCallback(
    async (title = 'New Chat') => {
      try {
        const created = await createMutation.mutateAsync({ title });
        createSessionWithId(created.id, created.title);
        return created.id;
      } catch {
        // Fallback: create locally only
        createSessionLocal();
        return useViewStore.getState().currentSessionId;
      }
    },
    [createMutation, createSessionWithId, createSessionLocal],
  );

  /** Delete from DB, then remove from viewStore. */
  const deleteSessionWithSync = useCallback(
    async (id: string) => {
      try {
        await deleteMutation.mutateAsync(id);
      } catch {
        // Best-effort: still delete locally
      }
      deleteSessionLocal(id);
    },
    [deleteMutation, deleteSessionLocal],
  );

  /** Rename in DB, then update viewStore. */
  const renameSessionWithSync = useCallback(
    async (id: string, title: string) => {
      updateSessionTitleLocal(id, title);
      try {
        await updateMutation.mutateAsync({ id, title });
      } catch {
        // Local update already applied; best-effort
      }
    },
    [updateMutation, updateSessionTitleLocal],
  );

  /** Persist a message to the DB for the given session. */
  const addMessageWithSync = useCallback(
    async (params: {
      sessionId: string;
      role: string;
      content: string;
      model?: string;
      agent?: string;
    }) => {
      try {
        await addMessageMutation.mutateAsync(params);
      } catch {
        // Message already in localStorage via viewStore.addMessage; best-effort
      }
    },
    [addMessageMutation],
  );

  return {
    sessions,
    currentSessionId,
    selectSession,
    createSessionWithSync,
    deleteSessionWithSync,
    renameSessionWithSync,
    addMessageWithSync,
    isLoading: createMutation.isPending,
  };
}
