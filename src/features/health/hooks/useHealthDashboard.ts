/** Jaskier Shared Pattern */
// src/features/health/hooks/useHealthDashboard.ts
/**
 * GeminiHydra v15 - Health Dashboard Hook
 * =========================================
 * Aggregates health, auth mode, system stats, and model count
 * for the HealthDashboard component. Reuses existing TanStack Query hooks
 * where available and adds new lightweight queries.
 *
 * Backend endpoints used:
 *   GET /api/health        → { status, version, app, uptime_seconds, providers[] }
 *   GET /api/system/stats  → { cpu_usage_percent, memory_used_mb, memory_total_mb, platform }
 *   GET /api/auth/mode     → { auth_required }
 *   GET /api/models        → { total_models, selected: { chat, thinking, image }, ... }
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

/** Shape returned by GET /api/models (model_registry::list_models) */
interface ModelsResponse {
  total_models: number;
  cache_stale: boolean;
  cache_age_seconds: number | null;
  pins: Record<string, string>;
  selected: {
    chat: SelectedModel | null;
    thinking: SelectedModel | null;
    image: SelectedModel | null;
  };
  providers: Record<string, unknown[]>;
}

interface SelectedModel {
  id: string;
  display_name?: string;
  [key: string]: unknown;
}

interface ResolvedModels {
  chat: string | null;
  thinking: string | null;
  image: string | null;
}

export interface HealthDashboardData {
  backendOnline: boolean;
  uptimeSeconds: number | null;
  authRequired: boolean | null;
  cpuUsage: number | null;
  memoryUsedMb: number | null;
  memoryTotalMb: number | null;
  modelCount: number | null;
  /** Active model assignments from the dynamic registry */
  resolvedModels: ResolvedModels | null;
  /** Last watchdog check timestamp (ISO string) */
  watchdogLastCheck: string | null;
  /** Watchdog status */
  watchdogStatus: string | null;
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

  const modelsQuery = useQuery<ModelsResponse>({
    queryKey: ['models', 'list'],
    queryFn: () => apiGetPolling<ModelsResponse>('/api/models'),
    refetchInterval: 60_000,
    retry: false, // refetchInterval handles recovery
    enabled: backendOnline, // don't poll when backend is down
  });

  // Uptime comes from /api/health (uptime_seconds field)
  const uptimeSeconds = healthQuery.data?.uptime_seconds ?? null;
  const authRequired = authQuery.data?.auth_required ?? null;
  // System stats uses backend field names: cpu_usage_percent, memory_used_mb, memory_total_mb
  const cpuUsage = statsQuery.data?.cpu_usage_percent ?? null;
  const memoryUsedMb = statsQuery.data?.memory_used_mb ?? null;
  const memoryTotalMb = statsQuery.data?.memory_total_mb ?? null;
  // Model count from total_models field
  const modelCount = modelsQuery.data?.total_models ?? null;

  // Resolved models from /api/models selected field
  const selected = modelsQuery.data?.selected ?? null;
  const resolvedModels: ResolvedModels | null = selected
    ? {
        chat: selected.chat?.id ?? null,
        thinking: selected.thinking?.id ?? null,
        image: selected.image?.id ?? null,
      }
    : null;

  // Watchdog status: backend is running if health is ok
  // (no separate watchdog endpoint — derive from health status)
  const watchdogLastCheck: string | null = null;
  const watchdogStatus: string | null = backendOnline
    ? healthQuery.data?.status === 'ok'
      ? 'ok'
      : (healthQuery.data?.status ?? null)
    : null;

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
    resolvedModels,
    watchdogLastCheck,
    watchdogStatus,
    loading,
    error,
    refetch,
  };
}
