/** Jaskier Shared Pattern */
// src/features/health/components/HealthDashboard.tsx
/**
 * GeminiHydra v15 - Health Dashboard
 * ====================================
 * Comprehensive monitoring grid showing backend status, auth mode,
 * system resources, model cache, uptime, WebSocket status,
 * active model assignments, and watchdog report.
 */

import { Activity, Bot, Clock, Cpu, Database, Radio, RefreshCw, Shield, Wifi } from 'lucide-react';
import { memo, type ReactNode, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Card } from '@/components/atoms/Card';
import { QueryError } from '@/components/molecules/QueryError';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { useHealthDashboard } from '../hooks/useHealthDashboard';

// ============================================================================
// HELPERS
// ============================================================================

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${String(d)}d ${String(h)}h ${String(m)}m`;
  if (h > 0) return `${String(h)}h ${String(m)}m`;
  return `${String(m)}m`;
}

function formatMemory(usedMb: number, totalMb: number): string {
  return `${String(Math.round(usedMb))} / ${String(Math.round(totalMb))} MB`;
}

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString(navigator.language, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return iso;
  }
}

// ============================================================================
// STAT CARD SUB-COMPONENT
// ============================================================================

interface StatCardProps {
  icon: ReactNode;
  label: string;
  value: string;
  statusColor?: string;
  subtitle?: string;
}

const StatCard = memo<StatCardProps>(({ icon, label, value, statusColor, subtitle }) => {
  const theme = useViewTheme();

  return (
    <Card variant="default" padding="sm" className="flex items-center gap-3 min-w-0">
      <div className={cn('flex-shrink-0', theme.iconMuted)}>{icon}</div>
      <div className="flex-1 min-w-0">
        <p className={cn('text-[10px] uppercase tracking-wider font-mono', theme.textMuted)}>{label}</p>
        <p className={cn('text-sm font-mono font-semibold truncate', statusColor ?? theme.text)}>{value}</p>
        {subtitle && <p className={cn('text-[10px] font-mono truncate', theme.textMuted)}>{subtitle}</p>}
      </div>
    </Card>
  );
});

StatCard.displayName = 'StatCard';

// ============================================================================
// HEALTH DASHBOARD
// ============================================================================

export const HealthDashboard = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const data = useHealthDashboard();
  const [refreshing, setRefreshing] = useState(false);

  const handleRefresh = useCallback(() => {
    setRefreshing(true);
    data.refetch();
    // Reset animation after 1s
    setTimeout(() => setRefreshing(false), 1000);
  }, [data]);

  if (data.loading) {
    return (
      <div className="w-full">
        <h3 className={cn('text-sm font-mono font-semibold uppercase tracking-wider mb-3', theme.textMuted)}>
          {t('health.title', 'System Health')}
        </h3>
        <div className={cn('text-sm font-mono animate-pulse', theme.textMuted)}>
          {t('common.loading', 'Loading...')}
        </div>
      </div>
    );
  }

  if (data.error) {
    return (
      <div className="w-full">
        <h3 className={cn('text-sm font-mono font-semibold uppercase tracking-wider mb-3', theme.textMuted)}>
          {t('health.title', 'System Health')}
        </h3>
        <QueryError onRetry={data.refetch} />
      </div>
    );
  }

  // Determine active model display
  const activeModelLabel = data.resolvedModels
    ? [
        data.resolvedModels.chat ? `Chat: ${data.resolvedModels.chat}` : null,
        data.resolvedModels.thinking ? `Think: ${data.resolvedModels.thinking}` : null,
        data.resolvedModels.image ? `Img: ${data.resolvedModels.image}` : null,
      ]
        .filter(Boolean)
        .join(' | ') || '--'
    : '--';

  return (
    <section className="w-full" aria-label={t('health.title', 'System Health')}>
      <div className="flex items-center justify-between mb-3">
        <h3 className={cn('text-sm font-mono font-semibold uppercase tracking-wider', theme.textMuted)}>
          {t('health.title', 'System Health')}
        </h3>
        <button
          type="button"
          onClick={handleRefresh}
          className={cn('p-1.5 rounded-lg transition-all', 'hover:bg-[var(--matrix-accent)]/10', theme.textMuted)}
          title={t('health.refresh', 'Refresh all metrics')}
          aria-label={t('health.refresh', 'Refresh all metrics')}
        >
          <RefreshCw size={14} className={refreshing ? 'animate-spin' : ''} />
        </button>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
        {/* Backend Status */}
        <StatCard
          icon={<Activity size={16} />}
          label={t('health.backend', 'Backend')}
          value={data.backendOnline ? t('health.online', 'Online') : t('health.offline', 'Offline')}
          statusColor={data.backendOnline ? 'text-emerald-400' : 'text-red-400'}
        />

        {/* Auth Mode */}
        <StatCard
          icon={<Shield size={16} />}
          label={t('health.auth', 'Authentication')}
          value={
            data.authRequired === null
              ? '--'
              : data.authRequired
                ? t('health.enabled', 'Enabled')
                : t('health.devMode', 'Dev Mode')
          }
        />

        {/* CPU Usage */}
        <StatCard
          icon={<Cpu size={16} />}
          label={t('health.cpu', 'CPU Usage')}
          value={data.cpuUsage !== null ? `${String(Math.round(data.cpuUsage))}%` : '--'}
          statusColor={
            data.cpuUsage !== null
              ? data.cpuUsage > 80
                ? 'text-red-400'
                : data.cpuUsage > 50
                  ? 'text-yellow-400'
                  : undefined
              : undefined
          }
        />

        {/* Memory */}
        <StatCard
          icon={<Cpu size={14} />}
          label={t('health.memory', 'Memory')}
          value={
            data.memoryUsedMb !== null && data.memoryTotalMb !== null
              ? formatMemory(data.memoryUsedMb, data.memoryTotalMb)
              : '--'
          }
          subtitle={
            data.memoryUsedMb !== null && data.memoryTotalMb !== null
              ? `${String(Math.round((data.memoryUsedMb / data.memoryTotalMb) * 100))}% used`
              : undefined
          }
        />

        {/* Model Cache */}
        <StatCard
          icon={<Database size={16} />}
          label={t('health.models', 'Cached Models')}
          value={data.modelCount !== null ? String(data.modelCount) : '--'}
        />

        {/* Uptime */}
        <StatCard
          icon={<Clock size={16} />}
          label={t('health.uptime', 'Uptime')}
          value={data.uptimeSeconds !== null ? formatUptime(data.uptimeSeconds) : '--'}
        />

        {/* WebSocket Status */}
        <StatCard
          icon={<Wifi size={16} />}
          label={t('health.websocket', 'WebSocket')}
          value={data.backendOnline ? t('health.available', 'Available') : t('health.unavailable', 'Unavailable')}
          statusColor={data.backendOnline ? 'text-emerald-400' : 'text-red-400'}
        />

        {/* Active Model */}
        <StatCard
          icon={<Bot size={16} />}
          label={t('health.activeModel', 'Active Models')}
          value={data.resolvedModels ? t('health.configured', 'Configured') : '--'}
          subtitle={activeModelLabel !== '--' ? activeModelLabel : undefined}
        />

        {/* Watchdog */}
        <StatCard
          icon={<Radio size={16} />}
          label={t('health.watchdog', 'Watchdog')}
          value={
            data.watchdogStatus
              ? data.watchdogStatus === 'ok'
                ? t('health.healthy', 'Healthy')
                : data.watchdogStatus
              : data.backendOnline
                ? t('health.running', 'Running')
                : '--'
          }
          statusColor={
            data.watchdogStatus === 'ok' || data.backendOnline
              ? 'text-emerald-400'
              : data.watchdogStatus
                ? 'text-yellow-400'
                : undefined
          }
          subtitle={data.watchdogLastCheck ? `Last: ${formatTimestamp(data.watchdogLastCheck)}` : undefined}
        />
      </div>
    </section>
  );
});

HealthDashboard.displayName = 'HealthDashboard';

export default HealthDashboard;
