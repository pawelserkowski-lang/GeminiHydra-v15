// src/features/status/components/StatusView.tsx
/**
 * Status View
 * ===========
 * Real-time system health dashboard with CPU, memory, uptime, agents, and requests.
 * Uses existing hooks from useHealth.ts.
 */

import { Clock, Cpu, HardDrive, Server, Users, Zap } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Card, ProgressBar } from '@/components/atoms';
import { StatusIndicator } from '@/components/molecules/StatusIndicator';
import type { StatusState } from '@/components/molecules/StatusIndicator';
import { useDetailedHealthQuery, useSystemStatsQuery } from '@/features/health/hooks/useHealth';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// HELPERS
// ============================================================================

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

function formatBytes(bytes: number): string {
  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(0)} MB`;
}

function healthToStatus(status: string | undefined): StatusState {
  if (!status) return 'offline';
  if (status === 'healthy' || status === 'ok') return 'online';
  if (status === 'degraded') return 'pending';
  return 'error';
}

// ============================================================================
// STAT CARD
// ============================================================================

interface StatCardProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  progress?: number;
  isLight: boolean;
}

function StatCard({ icon, label, value, progress, isLight }: StatCardProps) {
  return (
    <Card variant="glass" padding="md">
      <div className="flex items-center gap-3 mb-3">
        <div className={cn('p-2 rounded-lg', isLight ? 'bg-emerald-500/10' : 'bg-white/5')}>
          {icon}
        </div>
        <span className={cn('text-sm font-medium', isLight ? 'text-slate-600' : 'text-white/60')}>
          {label}
        </span>
      </div>
      <p className={cn('text-2xl font-bold font-mono', isLight ? 'text-slate-900' : 'text-white')}>
        {value}
      </p>
      {progress !== undefined && (
        <div className="mt-3">
          <ProgressBar value={progress} size="sm" />
        </div>
      )}
    </Card>
  );
}

// ============================================================================
// COMPONENT
// ============================================================================

export default function StatusView() {
  const { t } = useTranslation();
  const theme = useViewTheme();

  const { data: health, isLoading: healthLoading } = useDetailedHealthQuery();
  const { data: stats } = useSystemStatsQuery();

  // Merge data — prefer stats for real-time, health for version/status
  const cpuUsage = stats?.cpu_usage ?? health?.system?.cpu_usage ?? 0;
  const memoryUsed = stats?.memory_used ?? health?.system?.memory_used ?? 0;
  const memoryTotal = stats?.memory_total ?? health?.system?.memory_total ?? 1;
  const memoryPercent = memoryTotal > 0 ? (memoryUsed / memoryTotal) * 100 : 0;
  const uptimeSeconds = stats?.uptime_seconds ?? health?.uptime_seconds ?? 0;
  const activeAgents = stats?.active_agents ?? 0;
  const totalRequests = stats?.total_requests ?? 0;
  const platform = health?.system?.os ?? 'N/A';

  const statusState = useMemo(() => healthToStatus(health?.status), [health?.status]);
  const statusLabel = useMemo(() => {
    if (statusState === 'online') return t('status.health.healthy');
    if (statusState === 'pending') return t('status.health.degraded');
    return t('status.health.offline');
  }, [statusState, t]);

  const iconClass = cn('w-5 h-5', theme.isLight ? 'text-emerald-600' : 'text-white/70');

  // Loading state
  if (healthLoading && !health && !stats) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className={cn('text-lg font-mono animate-pulse', theme.isLight ? 'text-slate-600' : 'text-white/50')}>
          {t('common.loading')}
        </p>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto scrollbar-hide hover:scrollbar-thin hover:scrollbar-thumb-white/20 p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className={cn('text-2xl font-bold font-mono', theme.isLight ? 'text-slate-900' : 'text-white')}>
          {t('status.title')}
        </h1>
        <p className={cn('text-sm mt-1', theme.isLight ? 'text-slate-600' : 'text-white/50')}>
          {t('status.subtitle')}
        </p>
      </div>

      {/* Health Overview Card */}
      <Card variant="glass" padding="lg">
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div className="flex items-center gap-4">
            <StatusIndicator status={statusState} size="md" label={statusLabel} />
            {health?.version && (
              <Badge variant="accent" size="sm">
                {t('status.version')}: {health.version}
              </Badge>
            )}
          </div>
          <Badge variant="default" size="sm" dot>
            {t('status.connection')}: {t('status.websocket')} / {t('status.http')}
          </Badge>
        </div>
      </Card>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {/* CPU Usage */}
        <StatCard
          icon={<Cpu className={iconClass} />}
          label={t('status.stats.cpu')}
          value={`${cpuUsage.toFixed(1)}%`}
          progress={cpuUsage}
          isLight={theme.isLight}
        />

        {/* Memory */}
        <StatCard
          icon={<HardDrive className={iconClass} />}
          label={t('status.stats.memory')}
          value={`${formatBytes(memoryUsed)} / ${formatBytes(memoryTotal)}`}
          progress={memoryPercent}
          isLight={theme.isLight}
        />

        {/* Uptime */}
        <StatCard
          icon={<Clock className={iconClass} />}
          label={t('status.stats.uptime')}
          value={formatUptime(uptimeSeconds)}
          isLight={theme.isLight}
        />

        {/* Active Agents */}
        <StatCard
          icon={<Users className={iconClass} />}
          label={t('status.stats.active_agents')}
          value={String(activeAgents)}
          isLight={theme.isLight}
        />

        {/* Total Requests */}
        <StatCard
          icon={<Zap className={iconClass} />}
          label={t('status.stats.total_requests')}
          value={totalRequests.toLocaleString()}
          isLight={theme.isLight}
        />

        {/* Platform */}
        <StatCard
          icon={<Server className={iconClass} />}
          label={t('status.stats.platform')}
          value={platform}
          isLight={theme.isLight}
        />
      </div>
    </div>
  );
}
