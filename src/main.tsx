// src/main.tsx
/**
 * GeminiHydra v15 - Application Entry Point
 * ============================================
 * Wires: QueryClientProvider, ErrorBoundary, AppShell, ViewRouter, Toaster, i18n.
 * Phase 7: Views are lazy-loaded with React.lazy + Suspense for code-splitting.
 */

import { QueryClientProvider } from '@tanstack/react-query';
import { lazy, StrictMode, Suspense, useCallback, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Toaster } from 'sonner';
import { ViewSkeleton } from '@/components/molecules/ViewSkeleton';
import { AppShell } from '@/components/organisms/AppShell';
import { ErrorBoundary } from '@/components/organisms/ErrorBoundary';
import { useChatExecuteMutation } from '@/features/chat/hooks/useChat';
import { queryClient } from '@/shared/api/queryClient';
import { useViewStore } from '@/stores/viewStore';
import '@/i18n';
import './styles/globals.css';

// ============================================================================
// LAZY-LOADED VIEWS
// ============================================================================

const LazyWelcomeScreen = lazy(() => import('@/features/home/components/WelcomeScreen'));
const LazyChatContainer = lazy(() => import('@/features/chat/components/ChatContainer'));
const LazyAgentsView = lazy(() => import('@/features/agents/components/AgentsView'));
const LazyHistoryView = lazy(() => import('@/features/history/components/HistoryView'));

// ============================================================================
// CHAT VIEW WRAPPER
// ============================================================================

/**
 * ChatContainer requires isStreaming/onSubmit/onStop props.
 * This wrapper wires the useChatExecuteMutation hook to those props.
 * Kept in main.tsx (small) — the actual ChatContainer is lazy-loaded.
 */
function ChatViewWrapper() {
  const executeMutation = useChatExecuteMutation();
  const addMessage = useViewStore((s) => s.addMessage);
  const [isStreaming, setIsStreaming] = useState(false);

  const handleSubmit = useCallback(
    (prompt: string, _image: string | null) => {
      addMessage({ role: 'user', content: prompt, timestamp: Date.now() });
      setIsStreaming(true);

      executeMutation.mutate(
        { message: prompt },
        {
          onSuccess: (data) => {
            addMessage({
              role: 'assistant',
              content: data.response,
              timestamp: Date.now(),
              model: data.model,
            });
            setIsStreaming(false);
          },
          onError: () => {
            addMessage({
              role: 'assistant',
              content: 'An error occurred while generating a response.',
              timestamp: Date.now(),
            });
            setIsStreaming(false);
          },
        },
      );
    },
    [executeMutation, addMessage],
  );

  const handleStop = useCallback(() => {
    setIsStreaming(false);
  }, []);

  return <LazyChatContainer isStreaming={isStreaming} onSubmit={handleSubmit} onStop={handleStop} />;
}

// ============================================================================
// VIEW ROUTER
// ============================================================================

function ViewRouter() {
  const currentView = useViewStore((s) => s.currentView);

  switch (currentView) {
    case 'home':
      return <LazyWelcomeScreen />;
    case 'chat':
      return <ChatViewWrapper />;
    case 'agents':
      return <LazyAgentsView />;
    case 'history':
      return <LazyHistoryView />;
    case 'settings':
      return <div className="p-6 text-[var(--matrix-text-primary)]">Settings — Coming Soon</div>;
    case 'status':
      return <div className="p-6 text-[var(--matrix-text-primary)]">System Status — Coming Soon</div>;
  }
}

// ============================================================================
// APP
// ============================================================================

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary>
        <AppShell>
          <Suspense fallback={<ViewSkeleton />}>
            <ViewRouter />
          </Suspense>
        </AppShell>
      </ErrorBoundary>
      <Toaster position="bottom-right" theme="dark" richColors />
    </QueryClientProvider>
  );
}

// ============================================================================
// MOUNT
// ============================================================================

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
