// src/components/organisms/AppShell.tsx
/**
 * AppShell - Main layout composition
 * ====================================
 * Composes: ThemedBackground + RuneRain + Sidebar + TabBar + Content + StatusFooter.
 * Wrapped in ThemeProvider.
 * View transition animations are handled by ViewRouter in main.tsx.
 */

import { type ReactNode, useCallback, useEffect, useMemo } from 'react';
import { RuneRain, ThemedBackground } from '@/components/atoms';
import { CommandPalette } from '@/components/molecules/CommandPalette';
import { Sidebar } from '@/components/organisms/Sidebar';
import type { ConnectionHealth, StatusFooterProps } from '@/components/organisms/StatusFooter';
import { StatusFooter } from '@/components/organisms/StatusFooter';
import { TabBar } from '@/components/organisms/TabBar';
import { ThemeProvider, useTheme } from '@/contexts/ThemeContext';
import { useHealthStatus, useSystemStatsQuery } from '@/features/health/hooks/useHealth';
import { useSettingsQuery } from '@/features/settings/hooks/useSettings';
import { cn } from '@/shared/utils/cn';
import { useViewStore } from '@/stores/viewStore';

/** Format raw model ID (e.g. "gemini-3.1-pro-preview") into a display name ("Gemini 3.1 Pro"). */
function formatModelName(id: string): string {
  if (id.startsWith('ollama:')) return `Ollama: ${id.slice(7)}`;
  // Strip common suffixes; split into parts: "gemini-3.1-pro" → ["gemini", "3.1", "pro"]
  const parts = id
    .replace(/-preview$/, '')
    .replace(/-latest$/, '')
    .split('-');
  return parts.map((p) => (/^\d/.test(p) ? p : p.charAt(0).toUpperCase() + p.slice(1))).join(' ');
}

// ============================================================================
// TYPES
// ============================================================================

interface AppShellProps {
  /** Content to render in the main area */
  children: ReactNode;
  /** Props forwarded to the StatusFooter */
  statusFooterProps?: StatusFooterProps;
}

// ============================================================================
// INNER SHELL (needs ThemeProvider context)
// ============================================================================

function AppShellInner({ children, statusFooterProps }: AppShellProps) {
  const { resolvedTheme } = useTheme();
  const isLight = resolvedTheme === 'light';

  const currentView = useViewStore((s) => s.currentView);
  const activeModel = useViewStore((s) => s.activeModel);
  const { data: settings } = useSettingsQuery();
  const healthStatus = useHealthStatus();
  const { data: stats } = useSystemStatsQuery(healthStatus !== 'offline');

  let connectionHealth: ConnectionHealth;
  if (healthStatus === 'healthy') {
    connectionHealth = 'connected';
  } else if (healthStatus === 'degraded') {
    connectionHealth = 'degraded';
  } else {
    connectionHealth = 'disconnected';
  }

  // Resolve display model: WS activeModel → settings.default_model → fallback
  const displayModel = useMemo(() => {
    const raw = activeModel ?? settings?.default_model;
    return raw ? formatModelName(raw) : undefined;
  }, [activeModel, settings?.default_model]);

  // Build live footer props from system stats
  const resolvedFooterProps = useMemo<StatusFooterProps>(
    () => ({
      ...statusFooterProps,
      connectionHealth,
      ...(displayModel && { selectedModel: displayModel }),
      ...(stats && {
        cpuUsage: Math.round(stats.cpu_usage_percent),
        ramUsage: Math.round((stats.memory_used_mb / stats.memory_total_mb) * 100),
        statsLoaded: true,
      }),
    }),
    [statusFooterProps, connectionHealth, displayModel, stats],
  );

  // Global Ctrl+T shortcut — creates a new chat tab when in chat view
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 't' && currentView === 'chat') {
        e.preventDefault();
        useViewStore.getState().createSession();
        const sid = useViewStore.getState().currentSessionId;
        if (sid) useViewStore.getState().openTab(sid);
      }
    },
    [currentView],
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  return (
    <div
      className={cn(
        'relative flex h-screen w-full overflow-hidden font-mono transition-colors duration-500',
        isLight
          ? 'text-black selection:bg-emerald-500 selection:text-white'
          : 'text-white selection:bg-white/30 selection:text-white',
      )}
    >
      {/* Skip to content — accessibility */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:p-4 focus:bg-matrix-accent focus:text-white"
      >
        Skip to content
      </a>

      {/* Background layers */}
      <ThemedBackground resolvedTheme={resolvedTheme} />

      {/* RuneRain overlay */}
      <RuneRain opacity={0.12} />

      {/* Command Palette (Ctrl+K) */}
      <CommandPalette />

      {/* Main Content */}
      <div className="relative z-10 flex h-full w-full backdrop-blur-[1px] gap-4 p-4 overflow-hidden">
        {/* Sidebar */}
        <Sidebar />

        {/* Main Content Area */}
        <main
          id="main-content"
          data-testid="main-content"
          className={cn(
            'flex-1 min-w-0 flex flex-col overflow-hidden relative rounded-2xl',
            isLight
              ? 'bg-white/40 backdrop-blur-xl border border-white/20 shadow-lg'
              : 'bg-black/40 backdrop-blur-xl border border-white/10 shadow-2xl',
          )}
        >
          {/* Chat Tab Bar — only visible in chat view */}
          {currentView === 'chat' && <TabBar />}

          {/* View Content — animations handled by ViewRouter */}
          <div className="flex-1 min-h-0 overflow-hidden">{children}</div>

          {/* Status Footer */}
          <StatusFooter {...resolvedFooterProps} />
        </main>
      </div>
    </div>
  );
}

// ============================================================================
// APP SHELL (with ThemeProvider wrapper)
// ============================================================================

export function AppShell({ children, statusFooterProps }: AppShellProps) {
  return (
    <ThemeProvider defaultTheme="dark">
      <AppShellInner {...(statusFooterProps !== undefined && { statusFooterProps })}>{children}</AppShellInner>
    </ThemeProvider>
  );
}

AppShell.displayName = 'AppShell';
