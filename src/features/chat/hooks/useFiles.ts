// src/features/chat/hooks/useFiles.ts
/**
 * GeminiHydra v15 - File Access Hooks
 * =====================================
 * TanStack Query mutations for reading files and listing directories
 * via the backend file API.
 */

import { useMutation } from '@tanstack/react-query';
import { toast } from 'sonner';
import { apiPost } from '@/shared/api/client';
import type { FileReadResponse } from '@/shared/api/schemas';

interface FileReadInput {
  path: string;
}

export function useFileReadMutation() {
  return useMutation<FileReadResponse, Error, FileReadInput>({
    mutationFn: (input) => apiPost<FileReadResponse>('/api/files/read', input),
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}
