// src/features/history/hooks/useHistory.ts
/**
 * GeminiHydra v15 - History Hooks
 * =================================
 * TanStack Query hooks for chat history CRUD and search.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiDelete, apiGet, apiPost } from '@/shared/api/client';
import type { HistoryEntry, HistoryList } from '@/shared/api/schemas';

export function useHistoryQuery() {
  return useQuery<HistoryList>({
    queryKey: ['history'],
    queryFn: () => apiGet<HistoryList>('/api/history'),
  });
}

interface SearchHistoryInput {
  query: string;
  limit?: number;
}

export function useSearchHistoryMutation() {
  return useMutation<HistoryEntry[], Error, SearchHistoryInput>({
    mutationFn: (input) => apiPost<HistoryEntry[]>('/api/history/search', input),
  });
}

interface AddMessageInput {
  session_id: string;
  role: string;
  content: string;
  model?: string;
}

export function useAddMessageMutation() {
  const qc = useQueryClient();

  return useMutation<{ success: boolean }, Error, AddMessageInput>({
    mutationFn: (input) => apiPost<{ success: boolean }>('/api/history/message', input),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['history'] });
    },
  });
}

export function useClearHistoryMutation() {
  const qc = useQueryClient();

  return useMutation<{ success: boolean }, Error, void>({
    mutationFn: () => apiDelete<{ success: boolean }>('/api/history'),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['history'] });
    },
  });
}
