/**
 * Session management TanStack Query hooks for GeminiHydra v15.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { apiDelete, apiGet, apiPatch, apiPost } from '@/shared/api/client';
import type { Session, SessionSummary, SessionsList } from '@/shared/api/schemas';

/** GET /api/sessions */
export function useSessionsQuery() {
  return useQuery<SessionsList>({
    queryKey: ['sessions'],
    queryFn: () => apiGet<SessionsList>('/api/sessions'),
  });
}

/** POST /api/sessions */
export function useCreateSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation<Session, Error, { title?: string }>({
    mutationFn: (body) => apiPost<Session>('/api/sessions', body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sessions'] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}

/** PATCH /api/sessions/:id */
export function useUpdateSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation<SessionSummary, Error, { id: string; title: string }>({
    mutationFn: ({ id, title }) => apiPatch<SessionSummary>(`/api/sessions/${id}`, { title }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sessions'] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}

/** DELETE /api/sessions/:id */
export function useDeleteSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation<{ success: boolean }, Error, string>({
    mutationFn: (id) => apiDelete<{ success: boolean }>(`/api/sessions/${id}`),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sessions'] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}

/** POST /api/sessions/:id/messages */
export function useAddMessageMutation() {
  const queryClient = useQueryClient();
  return useMutation<
    { success: boolean },
    Error,
    { sessionId: string; role: string; content: string; model?: string; agent?: string }
  >({
    mutationFn: ({ sessionId, ...body }) => apiPost<{ success: boolean }>(`/api/sessions/${sessionId}/messages`, body),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: ['session', variables.sessionId] });
      void queryClient.invalidateQueries({ queryKey: ['sessions'] });
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}
