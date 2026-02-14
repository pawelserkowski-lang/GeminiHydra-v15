// src/features/chat/hooks/useChat.ts
/**
 * GeminiHydra v15 - Chat Hooks
 * ==============================
 * TanStack Query hooks for chat execution and Gemini model listing.
 */

import { useMutation, useQuery } from '@tanstack/react-query';
import { apiGet, apiPost } from '@/shared/api/client';
import type { ExecuteResponse, GeminiModels } from '@/shared/api/schemas';

interface ChatExecuteInput {
  prompt: string;
  mode: string;
  model?: string;
}

export function useChatExecuteMutation() {
  return useMutation<ExecuteResponse, Error, ChatExecuteInput>({
    mutationFn: (input) => apiPost<ExecuteResponse>('/api/execute', input),
  });
}

export function useGeminiModelsQuery() {
  return useQuery<GeminiModels>({
    queryKey: ['gemini', 'models'],
    queryFn: () => apiGet<GeminiModels>('/api/gemini/models'),
  });
}
