// src/features/chat/components/ChatViewWrapper.tsx
/**
 * ChatViewWrapper - Wires WebSocket/HTTP chat to ChatContainer
 * =============================================================
 * ChatContainer requires isStreaming/onSubmit/onStop props.
 * This wrapper wires the useWebSocketChat hook as primary path,
 * with useChatExecuteMutation as HTTP fallback when WS is unavailable.
 */

import { lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useChatExecuteMutation } from '@/features/chat/hooks/useChat';
import { useWebSocketChat, MAX_RECONNECT_ATTEMPTS } from '@/shared/hooks/useWebSocketChat';
import type { WsCallbacks } from '@/shared/hooks/useWebSocketChat';
import { useViewStore } from '@/stores/viewStore';

const LazyChatContainer = lazy(() => import('@/features/chat/components/ChatContainer'));

export function ChatViewWrapper() {
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

  const { status, streamingSessionId, connectionGaveUp, sendExecute, cancelStream, manualReconnect } =
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
                  ...(data.plan?.agent !== undefined && { model: data.plan.agent }),
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

  return (
    <>
      {connectionGaveUp && (
        <div className="flex items-center justify-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-sm mx-4 mt-2">
          <span className="text-red-400">Connection lost after {MAX_RECONNECT_ATTEMPTS} attempts</span>
          <button
            onClick={manualReconnect}
            className="px-3 py-1 rounded bg-red-500/20 hover:bg-red-500/30 transition-colors text-red-300"
          >
            Reconnect
          </button>
        </div>
      )}
      <LazyChatContainer isStreaming={isStreamingCurrentSession} onSubmit={handleSubmit} onStop={handleStop} />
    </>
  );
}

export default ChatViewWrapper;
