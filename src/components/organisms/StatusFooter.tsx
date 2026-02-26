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
// CONSTANTS
// ============================================================================

const HEALTH_MAP: Record<ConnectionHealth, { status: 'online' | 'pending' | 'offline'; label: string }> = {
  connected: { status: 'online', label: 'Online' },
  degraded: { status: 'pending', label: 'Degraded' },
  disconnected: { status: 'offline', label: 'Offline' },
};

const TIME_FORMAT: Intl.DateTimeFormatOptions = {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
};

function getUsageColor(
  usage: number,
  thresholdHigh: number,
  thresholdMid: number,
  normalLight: string,
  normalDark: string,
  isLight: boolean,
): string {
  if (usage > thresholdHigh) return 'text-red-400';
  if (usage > thresholdMid) return 'text-yellow-400';
  return isLight ? normalLight : normalDark;
}

function detectModelTier(model: string, isLight: boolean): { label: string; icon: typeof Cloud; cls: string } | null {
  const lower = model.toLowerCase();
  if (lower.includes('pro')) {
    return { label: 'PRO', icon: Cloud, cls: isLight ? 'text-blue-600' : 'text-blue-400' };
  }
  if (lower.includes('flash')) {
    return { label: 'FLASH', icon: Zap, cls: isLight ? 'text-amber-600' : 'text-amber-400' };
  }
  if (lower.includes('qwen') || lower.includes('llama')) {
    return { label: 'LOCAL', icon: Cpu, cls: isLight ? 'text-emerald-600' : 'text-emerald-400' };
  }
  return null;
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

  const [currentTime, setCurrentTime] = useState(() => new Date().toLocaleTimeString(navigator.language, TIME_FORMAT));

  useEffect(() => {
    const timer = setInterval(() => {
      setCurrentTime(new Date().toLocaleTimeString(navigator.language, TIME_FORMAT));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  const health = HEALTH_MAP[connectionHealth];
  const modelTier = detectModelTier(selectedModel, isLight);
  const cpuColor = getUsageColor(cpuUsage, 80, 50, 'text-sky-600', 'text-sky-400', isLight);
  const ramColor = getUsageColor(ramUsage, 85, 65, 'text-violet-600', 'text-violet-400', isLight);

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
          {new Date().toLocaleDateString(navigator.language, {
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
