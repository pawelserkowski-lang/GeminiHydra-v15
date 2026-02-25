/** Jaskier Shared Pattern */
// src/features/health/components/HealthDashboard.tsx
/**
 * GeminiHydra v15 - Health Dashboard
 * ====================================
 * Compact grid of stat cards showing backend status, auth mode,
 * system resources, model cache size, and uptime.
 */

import { Activity, Clock, Cpu, Database, Shield } from 'lucide-react';
import { memo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Card } from '@/components/atoms/Card';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { useHealthDashboard } from '../hooks/useHealthDashboard';

// ============================================================================
// HELPERS
// ============================================================================

function formatUptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${String(h)}h ${String(m)}m`;
  return `${String(m)}m`;
}

function formatMemory(usedMb: number, totalMb: number): string {
  return `${String(Math.round(usedMb))} / ${String(Math.round(totalMb))} MB`;
}

// ============================================================================
// STAT CARD SUB-COMPONENT
// ============================================================================

interface StatCardProps {
  icon: ReactNode;
  label: string;
  value: string;
  statusColor?: string;
}

const StatCard = memo<StatCardProps>(({ icon, label, value, statusColor }) => {
  const theme = useViewTheme();

  return (
    <Card variant="default" padding="sm" className="flex items-center gap-3 min-w-0">
      <div className={cn('flex-shrink-0', theme.iconMuted)}>{icon}</div>
      <div className="flex-1 min-w-0">
        <p className={cn('text-[10px] uppercase tracking-wider font-mono', theme.textMuted)}>{label}</p>
        <p className={cn('text-sm font-mono font-semibold truncate', statusColor ?? theme.text)}>{value}</p>
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

  return (
    <div className="w-full">
      <h3 className={cn('text-sm font-mono font-semibold uppercase tracking-wider mb-3', theme.textMuted)}>
        {t('health.title', 'System Health')}
      </h3>

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
      </div>
    </div>
  );
});

HealthDashboard.displayName = 'HealthDashboard';

export default HealthDashboard;
