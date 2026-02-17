// src/components/organisms/AppShell.tsx
/**
 * AppShell - Main layout composition
 * ====================================
 * Composes: LayeredBackground + WitcherRunes + Sidebar + TabBar + Content + StatusFooter.
 * Wrapped in ThemeProvider.
 * Ported pixel-perfect from GeminiHydra legacy App.tsx layout.
 *
 * Uses `motion` package (NOT framer-motion).
 */

import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, Suspense, useCallback, useEffect } from 'react';
import { LayeredBackground, WitcherRunes } from '@/components/atoms';
import { Sidebar } from '@/components/organisms/Sidebar';
import type { StatusFooterProps } from '@/components/organisms/StatusFooter';
import { StatusFooter } from '@/components/organisms/StatusFooter';
import { TabBar } from '@/components/organisms/TabBar';
import { ThemeProvider, useTheme } from '@/contexts/ThemeContext';
import { cn } from '@/shared/utils/cn';
import { useViewStore } from '@/stores/viewStore';

// ============================================================================
// TYPES
// ============================================================================

export interface AppShellProps {
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
  const isDark = resolvedTheme === 'dark';
  const isLight = resolvedTheme === 'light';

  const currentView = useViewStore((s) => s.currentView);

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
      className={`relative flex h-screen w-full ${isLight ? 'text-black selection:bg-emerald-500 selection:text-white' : 'text-white selection:bg-white/30 selection:text-white'} overflow-hidden font-mono transition-colors duration-500`}
    >
      {/* Background layers */}
      <LayeredBackground resolvedTheme={resolvedTheme} />

      {/* WitcherRunes overlay */}
      {import.meta.env.VITE_WITCHER_MODE !== 'disabled' && (
        <Suspense fallback={null}>
          <WitcherRunes isDark={isDark} />
        </Suspense>
      )}

      {/* Main Content */}
      <div className="relative z-10 flex h-full w-full backdrop-blur-[1px] gap-4 p-4 overflow-hidden">
        {/* Sidebar */}
        <Sidebar />

        {/* Main Content Area */}
        <main
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

          {/* View Content with animated transitions */}
          <div className="flex-1 min-h-0 overflow-hidden">
            <AnimatePresence mode="wait">
              <motion.div
                key={currentView}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                transition={{ duration: 0.2 }}
                className="h-full"
              >
                {children}
              </motion.div>
            </AnimatePresence>
          </div>

          {/* Status Footer */}
          <StatusFooter {...statusFooterProps} />
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
      <AppShellInner statusFooterProps={statusFooterProps}>{children}</AppShellInner>
    </ThemeProvider>
  );
}

AppShell.displayName = 'AppShell';
