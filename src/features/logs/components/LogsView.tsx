// src/features/logs/components/LogsView.tsx

import { useQueryClient } from '@tanstack/react-query';
import { ChevronDown, Copy, RefreshCw, ScrollText, Search, Trash2 } from 'lucide-react';
import { motion } from 'motion/react';
import { memo, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';

import { Badge, Button, Card, Input } from '@/components/atoms';
import { type BackendLogEntry, clearBackendLogs, useBackendLogs } from '@/features/logs/hooks/useLogs';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================
// LEVEL BADGE
// ============================================

function LevelBadge({ level }: { level: string }) {
  const upper = level.toUpperCase();
  const variant =
    upper === 'ERROR'
      ? 'error'
      : upper === 'WARN' || upper === 'WARNING'
        ? 'warning'
        : upper === 'INFO'
          ? 'accent'
          : 'default';

  return (
    <Badge variant={variant} size="sm" className="min-w-[52px] justify-center uppercase">
      {upper}
    </Badge>
  );
}

// ============================================
// TIMESTAMP FORMATTER
// ============================================

function formatTimestamp(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    });
  } catch {
    return ts;
  }
}

// ============================================
// MAIN VIEW
// ============================================

export const LogsView = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [level, setLevel] = useState('');
  const [autoRefresh, setAutoRefresh] = useState(true);

  const { data, isLoading, isError, refetch } = useBackendLogs(
    { limit: 200, level: level || undefined, search: search || undefined },
    autoRefresh,
  );

  const logs = data?.logs ?? [];

  const handleCopy = useCallback(async () => {
    if (!logs.length) {
      toast.error(t('logs.nothingToCopy', 'Nothing to copy'));
      return;
    }
    const text = logs.map((l) => `[${l.timestamp}] [${l.level}] ${l.target}: ${l.message}`).join('\n');
    await navigator.clipboard.writeText(text);
    toast.success(t('logs.copied', 'Copied to clipboard'));
  }, [logs, t]);

  const handleClear = useCallback(async () => {
    try {
      await clearBackendLogs();
      queryClient.invalidateQueries({ queryKey: ['logs-backend'] });
      toast.success(t('logs.cleared', 'Logs cleared'));
    } catch {
      toast.error(t('logs.clearError', 'Failed to clear logs'));
    }
  }, [queryClient, t]);

  return (
    <div className="h-full flex flex-col items-center p-8 overflow-y-auto">
      <motion.div
        className="w-full max-w-5xl space-y-6"
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4, ease: 'easeOut' }}
      >
        {/* Header */}
        <div className="flex items-center gap-3">
          <ScrollText size={22} className="text-[var(--matrix-accent)]" />
          <h1 className={cn('text-2xl font-bold font-mono tracking-tight', theme.title)}>{t('logs.title', 'Logs')}</h1>
          <div className="ml-auto flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={handleCopy} leftIcon={<Copy size={14} />}>
              {t('logs.copy', 'Copy')}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => void handleClear()} leftIcon={<Trash2 size={14} />}>
              {t('logs.clear', 'Clear')}
            </Button>
          </div>
        </div>

        {/* Filters + Content */}
        <Card>
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <div className="flex-1 min-w-[200px]">
                <Input
                  inputSize="sm"
                  icon={<Search size={14} />}
                  placeholder={t('logs.searchPlaceholder', 'Search logs...')}
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                />
              </div>
              <div className="relative">
                <select
                  value={level}
                  onChange={(e) => setLevel(e.target.value)}
                  className={cn(
                    'appearance-none font-mono text-xs px-3 py-1.5 pr-7 rounded-lg cursor-pointer',
                    theme.input,
                  )}
                >
                  <option value="">{t('logs.allLevels', 'All levels')}</option>
                  <option value="ERROR">ERROR</option>
                  <option value="WARN">WARN</option>
                  <option value="INFO">INFO</option>
                  <option value="DEBUG">DEBUG</option>
                  <option value="TRACE">TRACE</option>
                </select>
                <ChevronDown
                  size={12}
                  className={cn('absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none', theme.iconMuted)}
                />
              </div>
              <Button
                variant={autoRefresh ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setAutoRefresh(!autoRefresh)}
                leftIcon={<RefreshCw size={14} className={autoRefresh ? 'animate-spin' : ''} />}
              >
                {autoRefresh ? t('logs.autoOn', 'Auto') : t('logs.autoOff', 'Paused')}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => void refetch()} leftIcon={<RefreshCw size={14} />}>
                {t('logs.refresh', 'Refresh')}
              </Button>
            </div>

            {isLoading && (
              <p className={cn('text-sm font-mono text-center py-8', theme.textMuted)}>{t('common.loading')}</p>
            )}
            {isError && (
              <p className={cn('text-sm font-mono text-center py-8', theme.error)}>{t('common.loadError')}</p>
            )}

            {!isLoading && !isError && logs.length === 0 && (
              <p className={cn('text-sm font-mono text-center py-8', theme.empty)}>
                {t('logs.noLogs', 'No log entries')}
              </p>
            )}

            {logs.length > 0 && (
              <div className={cn('space-y-px rounded-lg overflow-hidden max-h-[65vh] overflow-y-auto', theme.border)}>
                {logs.map((log: BackendLogEntry, i: number) => (
                  <div
                    key={`${log.timestamp}-${i}`}
                    className={cn(
                      'flex items-start gap-2 px-3 py-1.5 font-mono text-xs',
                      i % 2 === 0 ? (theme.isLight ? 'bg-white/20' : 'bg-white/[0.02]') : 'bg-transparent',
                    )}
                  >
                    <span className={cn('flex-shrink-0 tabular-nums', theme.textMuted)}>
                      {formatTimestamp(log.timestamp)}
                    </span>
                    <LevelBadge level={log.level} />
                    <span className={cn('flex-shrink-0 max-w-[140px] truncate', theme.textMuted)}>{log.target}</span>
                    <span className={cn('flex-1 break-all', theme.text)}>{log.message}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </Card>
      </motion.div>
    </div>
  );
});

LogsView.displayName = 'LogsView';

export default LogsView;
