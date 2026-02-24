// src/features/health/hooks/useHealth.ts
/**
 * GeminiHydra v15 - Health Hooks
 * ===============================
 * TanStack Query hooks for health and system stats endpoints.
 */

import { useQuery } from '@tanstack/react-query';
import { apiGet } from '@/shared/api/client';
import type { DetailedHealth, SystemStats } from '@/shared/api/schemas';

export function useDetailedHealthQuery() {
  return useQuery<DetailedHealth>({
    queryKey: ['health', 'detailed'],
    queryFn: () => apiGet<DetailedHealth>('/api/health/detailed'),
  });
}

export function useSystemStatsQuery() {
  return useQuery<SystemStats>({
    queryKey: ['system', 'stats'],
    queryFn: () => apiGet<SystemStats>('/api/system/stats'),
    refetchInterval: 10_000,
  });
}
