// src/features/chat/hooks/useChat.ts
/**
 * GeminiHydra v15 - Chat Hooks
 * ==============================
 * TanStack Query hooks for chat execution and Gemini model listing.
 */

import { useMutation } from '@tanstack/react-query';
import { toast } from 'sonner';
import { apiPost } from '@/shared/api/client';
import type { ExecuteResponse } from '@/shared/api/schemas';

interface ChatExecuteInput {
  prompt: string;
  mode: string;
  model?: string;
}

export function useChatExecuteMutation() {
  return useMutation<ExecuteResponse, Error, ChatExecuteInput>({
    mutationFn: (input) => apiPost<ExecuteResponse>('/api/execute', input),
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}
