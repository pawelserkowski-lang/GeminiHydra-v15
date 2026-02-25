// src/features/settings/hooks/useSettings.ts
/**
 * GeminiHydra v15 - Settings Hooks
 * ==================================
 * TanStack Query hooks for reading settings.
 */

import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/shared/api/client';
import type { Settings } from '@/shared/api/schemas';

export function useSettingsQuery() {
  return useQuery<Settings>({
    queryKey: ['settings'],
    queryFn: () => apiGet<Settings>('/api/settings'),
  });
}
