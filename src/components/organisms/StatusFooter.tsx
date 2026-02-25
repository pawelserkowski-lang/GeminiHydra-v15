// src/components/organisms/StatusFooter.tsx
/** Jaskier Design System */
/**
 * StatusFooter - Compact status bar
 * ==================================
 * Displays: version, connection status, model tier, CPU%, RAM%,
 * agent name, and live time.
 * Ported pixel-perfect from GeminiHydra legacy App.tsx footer + StatusFooter.tsx.
 *
 * Uses `motion` package (NOT framer-motion).
 */

import { Cloud, Cpu, Zap } from 'lucide-react';
import { memo, useEffect, useState } from 'react';
import { StatusIndicator } from '@/components/molecules/StatusIndicator';
import { useTheme } from '@/contexts/ThemeContext';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

export type ConnectionHealth = 'connected' | 'degraded' | 'disconnected';

export interface StatusFooterProps {
  /** Connection health status */
  connectionHealth?: ConnectionHealth;
  /** Currently selected model name */
  selectedModel?: string;
  /** CPU usage percentage (0-100) — placeholder until backend connects */
  cpuUsage?: number;
  /** RAM usage percentage (0-100) — placeholder until backend connects */
  ramUsage?: number;
  /** App tagline / agent name */
  tagline?: string;
  /** Whether stats are loaded (from backend) */
  statsLoaded?: boolean;
}

// ============================================================================
// COMPONENT
// ============================================================================

const StatusFooterComponent = ({
  connectionHealth = 'connected',
  selectedModel = 'Gemini 3 Flash',
  cpuUsage = 12,
  ramUsage = 45,
  tagline = 'Multi-Agent AI Swarm',
  statsLoaded = true,
}: StatusFooterProps) => {
  const { resolvedTheme } = useTheme();
  const isLight = resolvedTheme === 'light';

  // Live time
  const [currentTime, setCurrentTime] = useState(() =>
    new Date().toLocaleTimeString('pl-PL', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );

  useEffect(() => {
    const timer = setInterval(() => {
      setCurrentTime(
        new Date().toLocaleTimeString('pl-PL', {
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit',
        }),
      );
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  // Connection status mapping
  const healthMap: Record<ConnectionHealth, { status: 'online' | 'pending' | 'offline'; label: string }> = {
    connected: { status: 'online', label: 'Online' },
    degraded: { status: 'pending', label: 'Degraded' },
    disconnected: { status: 'offline', label: 'Offline' },
  };

  const health = healthMap[connectionHealth];

  // Detect model tier
  const modelLower = selectedModel.toLowerCase();
  const modelTier = modelLower.includes('pro')
    ? { label: 'PRO', icon: Cloud, cls: isLight ? 'text-blue-600' : 'text-blue-400' }
    : modelLower.includes('flash')
      ? { label: 'FLASH', icon: Zap, cls: isLight ? 'text-amber-600' : 'text-amber-400' }
      : modelLower.includes('qwen') || modelLower.includes('llama')
        ? { label: 'LOCAL', icon: Cpu, cls: isLight ? 'text-emerald-600' : 'text-emerald-400' }
        : null;

  // CPU color based on usage
  const cpuColor =
    cpuUsage > 80 ? 'text-red-400' : cpuUsage > 50 ? 'text-yellow-400' : isLight ? 'text-sky-600' : 'text-sky-400';

  // RAM color based on usage
  const ramColor =
    ramUsage > 85
      ? 'text-red-400'
      : ramUsage > 65
        ? 'text-yellow-400'
        : isLight
          ? 'text-violet-600'
          : 'text-violet-400';

  const dividerCls = isLight ? 'text-slate-300' : 'text-white/20';

  return (
    <footer
      data-testid="status-footer"
      className={cn(
        'px-6 py-2.5 border-t text-sm flex items-center justify-between shrink-0 transition-all duration-500',
        isLight ? 'border-slate-200/30 bg-white/40 text-slate-600' : 'border-white/10 bg-black/20 text-slate-400',
      )}
    >
      {/* Left: Version + Connection + CPU + RAM */}
      <div className="flex items-center gap-4">
        {/* Version */}
        <span className={isLight ? 'text-emerald-600' : 'text-white'}>v15.0.0</span>

        <span className={dividerCls}>|</span>

        {/* Connection Status */}
        <StatusIndicator status={health.status} size="sm" label={health.label} />

        {/* CPU & RAM stats */}
        {statsLoaded && (
          <>
            <span className={dividerCls}>|</span>

            <span className={cn('font-semibold', cpuColor)} title={`CPU: ${cpuUsage}%`}>
              CPU {cpuUsage}%
            </span>

            <span className={cn('font-semibold', ramColor)} title={`RAM: ${ramUsage}%`}>
              RAM {ramUsage}%
            </span>
          </>
        )}
      </div>

      {/* Right: Model + Tier + Agent + Date + Time */}
      <div className="flex items-center gap-4">
        {/* Model tier badge */}
        {modelTier && (
          <div className={cn('flex items-center gap-1', modelTier.cls)}>
            <modelTier.icon size={10} aria-hidden="true" />
            <span className="font-bold">{modelTier.label}</span>
          </div>
        )}

        {/* Model name */}
        <span className={isLight ? 'text-slate-700' : 'text-white/50'}>{selectedModel}</span>

        <span className={dividerCls}>|</span>

        {/* Tagline */}
        <span>{tagline}</span>

        <span className={dividerCls}>|</span>

        {/* Date */}
        <span>
          {new Date().toLocaleDateString('pl-PL', {
            weekday: 'short',
            day: 'numeric',
            month: '2-digit',
            year: 'numeric',
          })}
        </span>

        <span className={dividerCls}>|</span>

        {/* Live time */}
        <span className={cn('font-mono font-semibold tabular-nums', isLight ? 'text-emerald-600' : 'text-white')}>
          {currentTime}
        </span>
      </div>
    </footer>
  );
};

StatusFooterComponent.displayName = 'StatusFooter';

export const StatusFooter = memo(StatusFooterComponent);
