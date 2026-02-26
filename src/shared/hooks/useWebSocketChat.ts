// src/shared/hooks/useWebSocketChat.ts
/**
 * GeminiHydra v15 — WebSocket Chat Hook
 * =======================================
 * Manages WebSocket lifecycle for streaming AI responses.
 * Auto-connect, reconnection with exponential backoff, heartbeat ping/pong.
 *
 * Heartbeat is paused while streaming (backend can't respond to pings during
 * execute_streaming) and reset on ANY incoming message to avoid killing the
 * connection during long tool-call loops.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import type {
  WsClientMessage,
  WsCompleteMessage,
  WsPlanMessage,
  WsServerMessage,
  WsStartMessage,
  WsToolCallMessage,
  WsToolResultMessage,
} from '@/shared/api/schemas';
import { wsServerMessageSchema } from '@/shared/api/schemas';
import { env } from '@/shared/config/env';

// ============================================================================
// TYPES
// ============================================================================

type WsStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface WsCallbacks {
  onStart?: (msg: WsStartMessage, sessionId: string | null) => void;
  onToken?: (content: string, sessionId: string | null) => void;
  onPlan?: (msg: WsPlanMessage, sessionId: string | null) => void;
  onToolCall?: (msg: WsToolCallMessage, sessionId: string | null) => void;
  onToolResult?: (msg: WsToolResultMessage, sessionId: string | null) => void;
  onComplete?: (msg: WsCompleteMessage, sessionId: string | null) => void;
  onError?: (message: string, sessionId: string | null) => void;
}

// ============================================================================
// CONSTANTS
// ============================================================================

const MAX_BACKOFF_MS = 16_000;
export const MAX_RECONNECT_ATTEMPTS = 10;
const HEARTBEAT_INTERVAL_MS = 30_000;
const HEARTBEAT_TIMEOUT_MS = 10_000;

function getWsUrl(): string {
  const backendUrl = env.VITE_BACKEND_URL;
  const authSecret = env.VITE_AUTH_SECRET;
  const tokenParam = authSecret ? `?token=${encodeURIComponent(authSecret)}` : '';

  if (backendUrl) {
    return `${backendUrl.replace(/^http/, 'ws')}/ws/execute${tokenParam}`;
  }

  const loc = window.location;
  const protocol = loc.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${loc.host}/ws/execute${tokenParam}`;
}

// ============================================================================
// HOOK
// ============================================================================

export function useWebSocketChat(callbacks: WsCallbacks) {
  const [status, setStatus] = useState<WsStatus>('disconnected');
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamingSessionId, setStreamingSessionId] = useState<string | null>(null);
  const [connectionGaveUp, setConnectionGaveUp] = useState(false);

  const wsRef = useRef<WebSocket | null>(null);
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

  const isStreamingRef = useRef(false);
  const streamingSessionIdRef = useRef<string | null>(null);
  const reconnectAttemptRef = useRef(0);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const heartbeatTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pongTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const intentionalCloseRef = useRef(false);

  const clearTimers = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    if (heartbeatTimerRef.current) {
      clearTimeout(heartbeatTimerRef.current);
      heartbeatTimerRef.current = null;
    }
    if (pongTimerRef.current) {
      clearTimeout(pongTimerRef.current);
      pongTimerRef.current = null;
    }
  }, []);

  const startHeartbeat = useCallback(() => {
    if (heartbeatTimerRef.current) clearTimeout(heartbeatTimerRef.current);
    if (pongTimerRef.current) {
      clearTimeout(pongTimerRef.current);
      pongTimerRef.current = null;
    }

    // Don't send heartbeat pings while streaming — backend blocks on
    // execute_streaming and can't respond to pings, which would cause
    // the pong timeout to kill the connection during long tool-call loops.
    if (isStreamingRef.current) return;

    heartbeatTimerRef.current = setTimeout(() => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) return;

      const ping: WsClientMessage = { type: 'ping' };
      ws.send(JSON.stringify(ping));

      pongTimerRef.current = setTimeout(() => {
        // No pong received — force reconnect
        ws.close();
      }, HEARTBEAT_TIMEOUT_MS);
    }, HEARTBEAT_INTERVAL_MS);
  }, []);

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    setStatus('connecting');
    const ws = new WebSocket(getWsUrl());
    wsRef.current = ws;

    ws.onopen = () => {
      setStatus('connected');
      reconnectAttemptRef.current = 0;
      startHeartbeat();
    };

    ws.onmessage = (event) => {
      // Reset heartbeat on ANY incoming message — proves connection is alive
      startHeartbeat();

      const raw = JSON.parse(event.data);
      const parsed = wsServerMessageSchema.safeParse(raw);
      if (!parsed.success) {
        // Silently ignore unknown message types (tool_call, tool_result, etc.)
        return;
      }

      const msg: WsServerMessage = parsed.data;
      const cbs = callbacksRef.current;

      const sid = streamingSessionIdRef.current;

      switch (msg.type) {
        case 'start':
          setIsStreaming(true);
          isStreamingRef.current = true;
          setStreamingSessionId(sid);
          cbs.onStart?.(msg, sid);
          break;
        case 'token':
          cbs.onToken?.(msg.content, sid);
          break;
        case 'plan':
          cbs.onPlan?.(msg, sid);
          break;
        case 'tool_call':
          cbs.onToolCall?.(msg, sid);
          break;
        case 'tool_result':
          cbs.onToolResult?.(msg, sid);
          break;
        case 'complete':
          setIsStreaming(false);
          isStreamingRef.current = false;
          setStreamingSessionId(null);
          cbs.onComplete?.(msg, sid);
          streamingSessionIdRef.current = null;
          startHeartbeat(); // Resume heartbeat after streaming ends
          break;
        case 'error':
          setIsStreaming(false);
          isStreamingRef.current = false;
          setStreamingSessionId(null);
          cbs.onError?.(msg.message, sid);
          streamingSessionIdRef.current = null;
          startHeartbeat();
          break;
        case 'pong':
          if (pongTimerRef.current) {
            clearTimeout(pongTimerRef.current);
            pongTimerRef.current = null;
          }
          startHeartbeat();
          break;
      }
    };

    ws.onclose = () => {
      setStatus('disconnected');
      setIsStreaming(false);
      isStreamingRef.current = false;
      setStreamingSessionId(null);
      streamingSessionIdRef.current = null;
      clearTimers();

      if (!intentionalCloseRef.current) {
        if (reconnectAttemptRef.current >= MAX_RECONNECT_ATTEMPTS) {
          setConnectionGaveUp(true);
          return;
        }
        // Add random jitter to prevent thundering herd on reconnect
        const baseDelay = Math.min(1000 * 2 ** reconnectAttemptRef.current, MAX_BACKOFF_MS);
        const delay = baseDelay + Math.random() * 1000;
        reconnectAttemptRef.current++;
        reconnectTimerRef.current = setTimeout(connect, delay);
      }
    };

    ws.onerror = () => {
      setStatus('error');
    };
  }, [startHeartbeat, clearTimers]);

  const disconnect = useCallback(() => {
    intentionalCloseRef.current = true;
    clearTimers();
    wsRef.current?.close();
    wsRef.current = null;
    setStatus('disconnected');
    setIsStreaming(false);
    isStreamingRef.current = false;
    setStreamingSessionId(null);
    streamingSessionIdRef.current = null;
  }, [clearTimers]);

  // Auto-connect on mount, cleanup on unmount
  useEffect(() => {
    intentionalCloseRef.current = false;
    connect();
    return disconnect;
  }, [connect, disconnect]);

  const sendExecute = useCallback((prompt: string, mode: string, model?: string, session_id?: string) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;

    // Track which session this stream belongs to
    streamingSessionIdRef.current = session_id ?? null;

    const msg: WsClientMessage = {
      type: 'execute',
      prompt,
      mode,
      ...(model !== undefined && { model }),
      ...(session_id !== undefined && { session_id }),
    };
    ws.send(JSON.stringify(msg));
  }, []);

  const cancelStream = useCallback(() => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;

    const msg: WsClientMessage = { type: 'cancel' };
    ws.send(JSON.stringify(msg));
    setIsStreaming(false);
    isStreamingRef.current = false;
    setStreamingSessionId(null);
    streamingSessionIdRef.current = null;
  }, []);

  const manualReconnect = useCallback(() => {
    reconnectAttemptRef.current = 0;
    setConnectionGaveUp(false);
    connect();
  }, [connect]);

  return { status, isStreaming, streamingSessionId, connectionGaveUp, sendExecute, cancelStream, manualReconnect };
}
