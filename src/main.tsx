// src/main.tsx
/**
 * GeminiHydra v15 - Application Entry Point
 * ============================================
 * Wires: QueryClientProvider, ErrorBoundary, AppShell, ViewRouter, Toaster, i18n.
 * Phase 7: Views are lazy-loaded with React.lazy + Suspense for code-splitting.
 */

import { QueryClientProvider, QueryErrorResetBoundary } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { AnimatePresence, motion } from 'motion/react';
import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { Toaster } from 'sonner';
import { FeatureErrorFallback } from '@/components/molecules/FeatureErrorFallback';
import { ViewSkeleton } from '@/components/molecules/ViewSkeleton';
import { AppShell } from '@/components/organisms/AppShell';
import { ErrorBoundary } from '@/components/organisms/ErrorBoundary';
import { ChatViewWrapper } from '@/features/chat/components/ChatViewWrapper';
import { queryClient } from '@/shared/api/queryClient';
import { reportWebVitals } from '@/shared/utils/reportWebVitals';
import { useViewStore } from '@/stores/viewStore';
import '@/i18n';
import './styles/globals.css';

// ============================================================================
// LAZY-LOADED VIEWS
// ============================================================================

const LazyWelcomeScreen = lazy(() => import('@/features/home/components/WelcomeScreen'));
const LazyAgentsView = lazy(() => import('@/features/agents/components/AgentsView'));
const LazyKnowledgeGraphView = lazy(() => import('@/features/memory/components/KnowledgeGraphView'));

// ============================================================================
// VIEW ROUTER
// ============================================================================

function ViewRouter() {
  const currentView = useViewStore((s) => s.currentView);

  function renderView() {
    switch (currentView) {
      case 'home':
        return <LazyWelcomeScreen />;
      case 'chat':
        return (
          <ErrorBoundary fallback={<FeatureErrorFallback feature="Chat" onRetry={() => window.location.reload()} />}>
            <ChatViewWrapper />
          </ErrorBoundary>
        );
      case 'agents':
        return (
          <ErrorBoundary fallback={<FeatureErrorFallback feature="Agents" onRetry={() => window.location.reload()} />}>
            <LazyAgentsView />
          </ErrorBoundary>
        );
      case 'brain':
        return (
          <ErrorBoundary
            fallback={<FeatureErrorFallback feature="Knowledge Graph" onRetry={() => window.location.reload()} />}
          >
            <LazyKnowledgeGraphView />
          </ErrorBoundary>
        );
    }
  }

  return (
    <div className="h-full overflow-hidden relative">
      <AnimatePresence mode="wait">
        <motion.div
          key={currentView}
          initial={{ opacity: 0, y: 6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -6 }}
          transition={{ duration: 0.2, ease: 'easeInOut' }}
          className="h-full w-full"
        >
          <QueryErrorResetBoundary>
            {({ reset }) => (
              <ErrorBoundary onReset={reset}>
                <Suspense fallback={<ViewSkeleton />}>{renderView()}</Suspense>
              </ErrorBoundary>
            )}
          </QueryErrorResetBoundary>
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

// ============================================================================
// APP
// ============================================================================

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <QueryErrorResetBoundary>
        {({ reset }) => (
          <ErrorBoundary onReset={reset}>
            <AppShell>
              <ViewRouter />
            </AppShell>
          </ErrorBoundary>
        )}
      </QueryErrorResetBoundary>
      <Toaster position="bottom-right" theme="dark" richColors />
      <ReactQueryDevtools initialIsOpen={false} />
    </QueryClientProvider>
  );
}

// ============================================================================
// MOUNT
// ============================================================================

// Jaskier Shared Pattern -- createRoot with HMR safety & documentation
/**
 * Application Mount Point
 * =======================
 * - React 19.2.4 + Vite 7 with Hot Module Replacement (HMR)
 * - StrictMode intentionally enabled in DEV for side-effect detection
 * - Double-renders in StrictMode are EXPECTED and INTENTIONAL (React 18+ behavior)
 * - This helps catch bugs in component lifecycle (effects, reducers, etc.)
 *
 * HMR Safety (Vite + @vitejs/plugin-react):
 * - import.meta.hot?.dispose() cleans up the root before HMR re-import
 * - Prevents "createRoot() on container already passed to createRoot()" error
 * - On code change: dispose() unmounts old tree → module re-imports → new createRoot()
 * - Production: import.meta.hot is undefined (Vite tree-shaking removes block)
 *
 * Reference: https://vitejs.dev/guide/ssr.html#setting-up-the-dev-server
 */

const root = document.getElementById('root');
if (root) {
  const appRoot = createRoot(root);
  appRoot.render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

  // HMR cleanup: unmount root before hot reload to prevent double-mount
  if (import.meta.hot) {
    import.meta.hot.dispose(() => {
      appRoot.unmount();
    });
  }

  // Report Web Vitals performance metrics
  reportWebVitals();
}
