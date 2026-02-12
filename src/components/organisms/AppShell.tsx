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
import { type ReactNode, Suspense } from 'react';
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

  const glassPanel = isLight ? 'glass-panel-light' : 'glass-panel-dark';

  return (
    <div className="relative flex h-screen w-full text-slate-100 overflow-hidden font-mono selection:bg-matrix-accent selection:text-black transition-colors duration-500">
      {/* Background layers */}
      <LayeredBackground resolvedTheme={resolvedTheme} />

      {/* WitcherRunes overlay */}
      <Suspense fallback={null}>
        <WitcherRunes isDark={isDark} />
      </Suspense>

      {/* Main Content */}
      <div className="relative z-10 flex h-full w-full backdrop-blur-[1px] gap-3 p-3 overflow-hidden">
        {/* Sidebar */}
        <Sidebar />

        {/* Main Content Area */}
        <main className={cn('flex-1 min-w-0 flex flex-col overflow-hidden relative rounded-2xl', glassPanel)}>
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
