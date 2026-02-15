// src/features/chat/hooks/useFiles.ts
/**
 * GeminiHydra v15 - File Access Hooks
 * =====================================
 * TanStack Query mutations for reading files and listing directories
 * via the backend file API.
 */

import { useMutation } from '@tanstack/react-query';
import { apiPost } from '@/shared/api/client';
import type { FileReadResponse, FileListResponse } from '@/shared/api/schemas';

interface FileReadInput {
  path: string;
}

interface FileListInput {
  path: string;
  show_hidden?: boolean;
}

export function useFileReadMutation() {
  return useMutation<FileReadResponse, Error, FileReadInput>({
    mutationFn: (input) => apiPost<FileReadResponse>('/api/files/read', input),
  });
}

export function useFileListMutation() {
  return useMutation<FileListResponse, Error, FileListInput>({
    mutationFn: (input) => apiPost<FileListResponse>('/api/files/list', input),
  });
}
