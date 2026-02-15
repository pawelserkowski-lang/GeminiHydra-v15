// src/shared/hooks/useWebSocketChat.ts
/**
 * GeminiHydra v15 — WebSocket Chat Hook
 * =======================================
 * Manages WebSocket lifecycle for streaming AI responses.
 * Auto-connect, reconnection with exponential backoff, heartbeat ping/pong.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import type {
  WsClientMessage,
  WsCompleteMessage,
  WsPlanMessage,
  WsServerMessage,
  WsStartMessage,
} from '@/shared/api/schemas';
import { wsServerMessageSchema } from '@/shared/api/schemas';

// ============================================================================
// TYPES
// ============================================================================

export type WsStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface WsCallbacks {
  onStart?: (msg: WsStartMessage) => void;
  onToken?: (content: string) => void;
  onPlan?: (msg: WsPlanMessage) => void;
  onComplete?: (msg: WsCompleteMessage) => void;
  onError?: (message: string) => void;
}

// ============================================================================
// CONSTANTS
// ============================================================================

const MAX_BACKOFF_MS = 16_000;
const HEARTBEAT_INTERVAL_MS = 30_000;
const HEARTBEAT_TIMEOUT_MS = 10_000;

function getWsUrl(): string {
  const loc = window.location;
  const protocol = loc.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${loc.host}/ws/execute`;
}

// ============================================================================
// HOOK
// ============================================================================

export function useWebSocketChat(callbacks: WsCallbacks) {
  const [status, setStatus] = useState<WsStatus>('disconnected');
  const [isStreaming, setIsStreaming] = useState(false);

  const wsRef = useRef<WebSocket | null>(null);
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

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
      const parsed = wsServerMessageSchema.safeParse(JSON.parse(event.data));
      if (!parsed.success) return;

      const msg: WsServerMessage = parsed.data;
      const cbs = callbacksRef.current;

      switch (msg.type) {
        case 'start':
          setIsStreaming(true);
          cbs.onStart?.(msg);
          break;
        case 'token':
          cbs.onToken?.(msg.content);
          break;
        case 'plan':
          cbs.onPlan?.(msg);
          break;
        case 'complete':
          setIsStreaming(false);
          cbs.onComplete?.(msg);
          break;
        case 'error':
          setIsStreaming(false);
          cbs.onError?.(msg.message);
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
      clearTimers();

      if (!intentionalCloseRef.current) {
        const delay = Math.min(
          1000 * 2 ** reconnectAttemptRef.current,
          MAX_BACKOFF_MS,
        );
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
  }, [clearTimers]);

  // Auto-connect on mount, cleanup on unmount
  useEffect(() => {
    intentionalCloseRef.current = false;
    connect();
    return disconnect;
  }, [connect, disconnect]);

  const sendExecute = useCallback(
    (prompt: string, mode: string, model?: string) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) return;

      const msg: WsClientMessage = { type: 'execute', prompt, mode, model };
      ws.send(JSON.stringify(msg));
    },
    [],
  );

  const cancelStream = useCallback(() => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;

    const msg: WsClientMessage = { type: 'cancel' };
    ws.send(JSON.stringify(msg));
    setIsStreaming(false);
  }, []);

  return { status, isStreaming, sendExecute, cancelStream };
}
