// src/features/health/hooks/useHealth.ts
/**
 * GeminiHydra v15 - Health Hooks
 * ===============================
 * TanStack Query hooks for health and system stats endpoints.
 */

import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/shared/api/client';
import type { SystemStats } from '@/shared/api/schemas';

export function useSystemStatsQuery() {
  return useQuery<SystemStats>({
    queryKey: ['system', 'stats'],
    queryFn: () => apiGet<SystemStats>('/api/system/stats'),
    refetchInterval: 10_000,
  });
}

export function useHealthQuery() {
  return useQuery<{ status: string }>({
    queryKey: ['health'],
    queryFn: () => apiGet<{ status: string }>('/api/health'),
    refetchInterval: 30_000,
  });
}

export function useHealthStatus(): 'healthy' | 'offline' | 'degraded' {
  const { data, isError } = useHealthQuery();
  if (isError || !data) return 'offline';
  const s = data.status?.toLowerCase();
  if (s === 'ok' || s === 'healthy') return 'healthy';
  return 'degraded';
}
