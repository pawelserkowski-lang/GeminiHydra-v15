/**
 * useSessionSync — Bridge between viewStore (localStorage) and backend DB.
 *
 * Provides synced CRUD operations that update both the local Zustand store
 * and the Postgres backend. On first load, hydrates localStorage from DB
 * if localStorage is empty.
 */

import { useCallback, useEffect, useRef } from 'react';
import { toast } from 'sonner';
import type { Session } from '@/stores/viewStore';
import { useViewStore } from '@/stores/viewStore';
import {
  useAddMessageMutation,
  useCreateSessionMutation,
  useDeleteSessionMutation,
  useGenerateTitleMutation,
  useSessionsQuery,
  useUpdateSessionMutation,
} from './useSessions';

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
  const syncWorkingDirectories = useViewStore((s) => s.syncWorkingDirectories);

  const { data: dbSessions, isSuccess: dbLoaded } = useSessionsQuery();

  const createMutation = useCreateSessionMutation();
  const deleteMutation = useDeleteSessionMutation();
  const updateMutation = useUpdateSessionMutation();
  const addMessageMutation = useAddMessageMutation();
  const generateTitleMutation = useGenerateTitleMutation();

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
        workingDirectory: s.working_directory ?? '',
      }));
      hydrateSessions(mapped);
      localStorage.setItem(MIGRATION_FLAG, 'true');
    } else if (!migrated) {
      localStorage.setItem(MIGRATION_FLAG, 'true');
    }
  }, [dbLoaded, dbSessions, sessions.length, hydrateSessions]);

  // Sync workingDirectory from DB on every load (DB is source of truth)
  useEffect(() => {
    if (!dbLoaded || !dbSessions) return;
    const dirs = dbSessions.map((s) => ({
      id: s.id,
      workingDirectory: s.working_directory ?? '',
    }));
    syncWorkingDirectories(dirs);
  }, [dbLoaded, dbSessions, syncWorkingDirectories]);

  /**
   * Optimistic session creation (#16).
   * Immediately adds a placeholder session to the store (visible in sidebar),
   * then replaces it with real DB data when the API responds.
   * If creation fails, removes the placeholder and shows an error toast.
   */
  const createSessionWithSync = useCallback(
    async (title = 'New Chat') => {
      // Step 1: Optimistically create a placeholder with a temporary UUID
      const placeholderId = `pending-${crypto.randomUUID()}`;
      createSessionWithId(placeholderId, title);

      try {
        // Step 2: Create in DB — get the real UUID
        const created = await createMutation.mutateAsync({ title });

        // Step 3: Replace placeholder with real session
        // Delete the placeholder and create with the real ID
        deleteSessionLocal(placeholderId);
        createSessionWithId(created.id, created.title);
        return created.id;
      } catch {
        // Step 3 (failure): Remove placeholder, show error toast
        deleteSessionLocal(placeholderId);
        toast.error('Failed to create session — using local fallback');
        // Fallback: create locally only
        createSessionLocal();
        return useViewStore.getState().currentSessionId;
      }
    },
    [createMutation, createSessionWithId, createSessionLocal, deleteSessionLocal],
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

  /** Ask AI to generate a session title from the first user message. */
  const generateTitleWithSync = useCallback(
    async (id: string) => {
      try {
        const result = await generateTitleMutation.mutateAsync(id);
        if (result.title) {
          updateSessionTitleLocal(id, result.title);
        }
      } catch {
        // Best-effort: substring title already set as placeholder
      }
    },
    [generateTitleMutation, updateSessionTitleLocal],
  );

  /** Persist a message to the DB for the given session. */
  const addMessageWithSync = useCallback(
    async (params: { sessionId: string; role: string; content: string; model?: string; agent?: string }) => {
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
    generateTitleWithSync,
    addMessageWithSync,
    isLoading: createMutation.isPending,
  };
}
