/** Jaskier Shared Pattern */
// src/features/health/hooks/useHealthDashboard.ts
/**
 * GeminiHydra v15 - Health Dashboard Hook
 * =========================================
 * Aggregates health, auth mode, system stats, and model count
 * for the HealthDashboard component. Reuses existing TanStack Query hooks
 * where available and adds new lightweight queries.
 */

import { useQuery } from '@tanstack/react-query';
import { apiGetPolling } from '@/shared/api/client';
import { useHealthQuery, useSystemStatsQuery } from './useHealth';

// ============================================================================
// TYPES
// ============================================================================

interface AuthMode {
  auth_required: boolean;
}

interface ModelInfo {
  id: string;
  [key: string]: unknown;
}

export interface HealthDashboardData {
  backendOnline: boolean;
  uptimeSeconds: number | null;
  authRequired: boolean | null;
  cpuUsage: number | null;
  memoryUsedMb: number | null;
  memoryTotalMb: number | null;
  modelCount: number | null;
  loading: boolean;
  error: boolean;
  refetch: () => void;
}

// ============================================================================
// HOOK
// ============================================================================

export function useHealthDashboard(): HealthDashboardData {
  const healthQuery = useHealthQuery();
  const backendOnline = !!healthQuery.data && !healthQuery.isError;

  const statsQuery = useSystemStatsQuery(backendOnline);

  const authQuery = useQuery<AuthMode>({
    queryKey: ['auth', 'mode'],
    queryFn: () => apiGetPolling<AuthMode>('/api/auth/mode'),
    refetchInterval: 60_000,
    retry: false, // refetchInterval handles recovery
    enabled: backendOnline, // don't poll when backend is down
  });

  const modelsQuery = useQuery<ModelInfo[]>({
    queryKey: ['models', 'list'],
    queryFn: () => apiGetPolling<ModelInfo[]>('/api/models'),
    refetchInterval: 60_000,
    retry: false, // refetchInterval handles recovery
    enabled: backendOnline, // don't poll when backend is down
  });
  // GeminiHydra useHealthQuery returns { status: string } without uptime,
  // so we read uptime_seconds from the SystemStats endpoint instead.
  const uptimeSeconds = statsQuery.data?.uptime_seconds ?? null;
  const authRequired = authQuery.data?.auth_required ?? null;
  const cpuUsage = statsQuery.data?.cpu_usage ?? null;
  const memoryUsedMb = statsQuery.data?.memory_used ?? null;
  const memoryTotalMb = statsQuery.data?.memory_total ?? null;
  const modelCount = modelsQuery.data ? modelsQuery.data.length : null;
  const loading = healthQuery.isLoading || statsQuery.isLoading;
  const error = healthQuery.isError && statsQuery.isError;

  const refetch = () => {
    void healthQuery.refetch();
    void statsQuery.refetch();
    void authQuery.refetch();
    void modelsQuery.refetch();
  };

  return {
    backendOnline,
    uptimeSeconds,
    authRequired,
    cpuUsage,
    memoryUsedMb,
    memoryTotalMb,
    modelCount,
    loading,
    error,
    refetch,
  };
}
