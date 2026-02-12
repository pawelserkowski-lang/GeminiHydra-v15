// src/features/settings/hooks/useSettings.ts
/**
 * GeminiHydra v15 - Settings Hooks
 * ==================================
 * TanStack Query hooks for reading, updating, and resetting settings.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiGet, apiPatch, apiPost } from '@/shared/api/client';
import type { Settings } from '@/shared/api/schemas';

export function useSettingsQuery() {
  return useQuery<Settings>({
    queryKey: ['settings'],
    queryFn: () => apiGet<Settings>('/api/settings'),
  });
}

export function useUpdateSettingsMutation() {
  const qc = useQueryClient();

  return useMutation<{ success: boolean; settings: Settings }, Error, Partial<Settings>>({
    mutationFn: (patch) => apiPatch<{ success: boolean; settings: Settings }>('/api/settings', patch),
    onSuccess: (data) => {
      qc.setQueryData(['settings'], data.settings);
    },
  });
}

export function useResetSettingsMutation() {
  const qc = useQueryClient();

  return useMutation<{ success: boolean; settings: Settings }, Error, void>({
    mutationFn: () => apiPost<{ success: boolean; settings: Settings }>('/api/settings/reset'),
    onSuccess: (data) => {
      qc.setQueryData(['settings'], data.settings);
    },
  });
}
