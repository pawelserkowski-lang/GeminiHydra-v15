// src/main.tsx
/**
 * GeminiHydra v15 - Application Entry Point
 * ============================================
 * Wires: QueryClientProvider, ErrorBoundary, AppShell, ViewRouter, Toaster, i18n.
 * Phase 7: Views are lazy-loaded with React.lazy + Suspense for code-splitting.
 */

import { QueryClientProvider } from '@tanstack/react-query';
import { AnimatePresence, motion } from 'motion/react';
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
  const addMessageToSession = useViewStore((s) => s.addMessageToSession);
  const updateLastMessageInSession = useViewStore((s) => s.updateLastMessageInSession);
  const currentSessionId = useViewStore((s) => s.currentSessionId);
  const [usingFallback, setUsingFallback] = useState(false);
  const fallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const httpStreamingSessionIdRef = useRef<string | null>(null);

  const wsCallbacks = useMemo<WsCallbacks>(
    () => ({
      onStart: (msg, sessionId) => {
        if (!sessionId) return;
        addMessageToSession(sessionId, {
          role: 'assistant',
          content: '',
          timestamp: Date.now(),
          model: msg.agent,
        });
        useViewStore.getState().setActiveModel(msg.model);
      },
      onToken: (content, sessionId) => {
        if (!sessionId) return;
        updateLastMessageInSession(sessionId, content);
      },
      onError: (message, sessionId) => {
        if (!sessionId) return;
        addMessageToSession(sessionId, {
          role: 'assistant',
          content: `Error: ${message}`,
          timestamp: Date.now(),
        });
      },
    }),
    [addMessageToSession, updateLastMessageInSession],
  );

  const { status, streamingSessionId, sendExecute, cancelStream } =
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

  const [httpStreamingSessionId, setHttpStreamingSessionId] = useState<string | null>(null);

  // Derive per-session streaming: only block input for the currently viewed session
  const isStreamingCurrentSession = usingFallback
    ? httpStreamingSessionId === currentSessionId
    : streamingSessionId === currentSessionId;

  const handleSubmit = useCallback(
    (prompt: string, _image: string | null) => {
      // Auto-create session if none exists
      if (!useViewStore.getState().currentSessionId) {
        useViewStore.getState().createSession();
        const sid = useViewStore.getState().currentSessionId;
        if (sid) useViewStore.getState().openTab(sid);
      }

      const sessionId = useViewStore.getState().currentSessionId;
      if (!sessionId) return;

      addMessageToSession(sessionId, { role: 'user', content: prompt, timestamp: Date.now() });

      if (!usingFallback && status === 'connected') {
        // Primary: WebSocket streaming (only when WS is actually connected)
        sendExecute(prompt, 'chat', undefined, sessionId);
      } else {
        // Fallback: HTTP — track which session is streaming
        setHttpStreamingSessionId(sessionId);
        httpStreamingSessionIdRef.current = sessionId;
        executeMutation.mutate(
          { prompt, mode: 'chat' },
          {
            onSuccess: (data) => {
              const sid = httpStreamingSessionIdRef.current;
              if (sid) {
                addMessageToSession(sid, {
                  role: 'assistant',
                  content: data.result,
                  timestamp: Date.now(),
                  model: data.plan?.agent,
                });
              }
              setHttpStreamingSessionId(null);
              httpStreamingSessionIdRef.current = null;
            },
            onError: () => {
              const sid = httpStreamingSessionIdRef.current;
              if (sid) {
                addMessageToSession(sid, {
                  role: 'assistant',
                  content: 'An error occurred while generating a response.',
                  timestamp: Date.now(),
                });
              }
              setHttpStreamingSessionId(null);
              httpStreamingSessionIdRef.current = null;
            },
          },
        );
      }
    },
    [addMessageToSession, usingFallback, status, sendExecute, executeMutation],
  );

  const handleStop = useCallback(() => {
    if (!usingFallback) {
      cancelStream();
    } else {
      setHttpStreamingSessionId(null);
      httpStreamingSessionIdRef.current = null;
    }
  }, [usingFallback, cancelStream]);

  return <LazyChatContainer isStreaming={isStreamingCurrentSession} onSubmit={handleSubmit} onStop={handleStop} />;
}

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
        return <ChatViewWrapper />;
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
          <ErrorBoundary>
            <Suspense fallback={<ViewSkeleton />}>{renderView()}</Suspense>
          </ErrorBoundary>
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
      <ErrorBoundary>
        <AppShell>
          <ViewRouter />
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
