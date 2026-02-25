// src/shared/api/queryClient.ts
/**
 * GeminiHydra v15 - TanStack Query Client
 * =========================================
 * Shared QueryClient instance with sensible defaults.
 */

import { QueryClient } from '@tanstack/react-query';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});
