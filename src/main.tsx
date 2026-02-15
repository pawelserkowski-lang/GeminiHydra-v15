// src/main.tsx
/**
 * GeminiHydra v15 - Application Entry Point
 * ============================================
 * Wires: QueryClientProvider, ErrorBoundary, AppShell, ViewRouter, Toaster, i18n.
 * Phase 7: Views are lazy-loaded with React.lazy + Suspense for code-splitting.
 */

import { QueryClientProvider } from '@tanstack/react-query';
import { lazy, StrictMode, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Toaster } from 'sonner';
import { ViewSkeleton } from '@/components/molecules/ViewSkeleton';
import { AppShell } from '@/components/organisms/AppShell';
import { ErrorBoundary } from '@/components/organisms/ErrorBoundary';
import { useChatExecuteMutation } from '@/features/chat/hooks/useChat';
import { queryClient } from '@/shared/api/queryClient';
import { useWebSocketChat } from '@/shared/hooks/useWebSocketChat';
import type { WsCallbacks } from '@/shared/hooks/useWebSocketChat';
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
 * This wrapper wires the useWebSocketChat hook as primary path,
 * with useChatExecuteMutation as HTTP fallback when WS is unavailable.
 */
function ChatViewWrapper() {
  const executeMutation = useChatExecuteMutation();
  const addMessage = useViewStore((s) => s.addMessage);
  const updateLastMessage = useViewStore((s) => s.updateLastMessage);
  const [usingFallback, setUsingFallback] = useState(false);
  const fallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const wsCallbacks = useMemo<WsCallbacks>(
    () => ({
      onStart: (msg) => {
        addMessage({
          role: 'assistant',
          content: '',
          timestamp: Date.now(),
          model: msg.agent,
        });
      },
      onToken: (content) => {
        updateLastMessage(content);
      },
      onError: (message) => {
        addMessage({
          role: 'assistant',
          content: `Error: ${message}`,
          timestamp: Date.now(),
        });
      },
    }),
    [addMessage, updateLastMessage],
  );

  const { status, isStreaming: wsStreaming, sendExecute, cancelStream } =
    useWebSocketChat(wsCallbacks);

  // Fallback: if WS never reaches 'connected' within 5s, switch to HTTP.
  // Only clear fallback when WS actually connects (not on reconnect attempts).
  useEffect(() => {
    if (status === 'connected') {
      if (fallbackTimerRef.current) clearTimeout(fallbackTimerRef.current);
      fallbackTimerRef.current = null;
      setUsingFallback(false);
    } else if (!fallbackTimerRef.current && !usingFallback) {
      fallbackTimerRef.current = setTimeout(() => setUsingFallback(true), 5000);
    }
    return () => {
      if (fallbackTimerRef.current) clearTimeout(fallbackTimerRef.current);
    };
  }, [status, usingFallback]);

  const [httpStreaming, setHttpStreaming] = useState(false);
  const isStreaming = usingFallback ? httpStreaming : wsStreaming;

  const handleSubmit = useCallback(
    (prompt: string, _image: string | null) => {
      // Auto-create session if none exists
      if (!useViewStore.getState().currentSessionId) {
        useViewStore.getState().createSession();
        const sid = useViewStore.getState().currentSessionId;
        if (sid) useViewStore.getState().openTab(sid);
      }
      addMessage({ role: 'user', content: prompt, timestamp: Date.now() });

      if (!usingFallback && status === 'connected') {
        // Primary: WebSocket streaming (only when WS is actually connected)
        sendExecute(prompt, 'chat');
      } else {
        // Fallback: HTTP
        setHttpStreaming(true);
        executeMutation.mutate(
          { prompt, mode: 'chat' },
          {
            onSuccess: (data) => {
              addMessage({
                role: 'assistant',
                content: data.result,
                timestamp: Date.now(),
                model: data.plan?.agent,
              });
              setHttpStreaming(false);
            },
            onError: () => {
              addMessage({
                role: 'assistant',
                content: 'An error occurred while generating a response.',
                timestamp: Date.now(),
              });
              setHttpStreaming(false);
            },
          },
        );
      }
    },
    [addMessage, usingFallback, status, sendExecute, executeMutation],
  );

  const handleStop = useCallback(() => {
    if (!usingFallback) {
      cancelStream();
    } else {
      setHttpStreaming(false);
    }
  }, [usingFallback, cancelStream]);

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
