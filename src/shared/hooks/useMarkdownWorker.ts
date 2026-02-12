// src/shared/hooks/useMarkdownWorker.ts
/**
 * useMarkdownWorker Hook
 * =======================
 * Manages a Web Worker for off-main-thread markdown parsing.
 * Caches results by content hash to avoid redundant work.
 * Falls back to synchronous parsing if the worker is unavailable.
 */

import { useCallback, useEffect, useRef } from 'react';
import type { MarkdownWorkerRequest, MarkdownWorkerResponse } from '@/workers/markdownWorker';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface PendingRequest {
  resolve: (html: string) => void;
  reject: (error: Error) => void;
}

// ---------------------------------------------------------------------------
// Simple String Hash (djb2)
// ---------------------------------------------------------------------------

function hashString(str: string): string {
  let hash = 5381;
  for (let i = 0; i < str.length; i++) {
    // eslint-disable-next-line no-bitwise
    hash = ((hash << 5) + hash + str.charCodeAt(i)) | 0;
  }
  return hash.toString(36);
}

// ---------------------------------------------------------------------------
// Synchronous Fallback Parser
// ---------------------------------------------------------------------------

function syncParseMarkdown(content: string): string {
  let html = content;

  // Escape HTML for safety in non-code contexts
  const escapeHtml = (text: string): string => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

  // Fenced code blocks
  html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_match, lang: string, code: string) => {
    const langAttr = lang ? ` data-language="${lang}"` : '';
    return `<pre${langAttr}><code>${escapeHtml(code.trimEnd())}</code></pre>`;
  });

  // Inline code
  html = html.replace(/`([^`\n]+)`/g, (_match, code: string) => `<code>${escapeHtml(code)}</code>`);

  // Bold
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');

  // Italic
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');

  // Paragraphs for plain lines
  html = html.replace(/^(?!<(?:h[1-6]|pre|blockquote|ul|ol|li|hr|div|p))(.+)$/gm, '<p>$1</p>');

  return html.trim();
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useMarkdownWorker(): {
  parseMarkdown: (content: string) => Promise<string>;
} {
  const workerRef = useRef<Worker | null>(null);
  const pendingRef = useRef<Map<string, PendingRequest>>(new Map());
  const cacheRef = useRef<Map<string, string>>(new Map());
  const idCounterRef = useRef(0);

  // Initialize worker
  useEffect(() => {
    try {
      const worker = new Worker(new URL('../../workers/markdownWorker.ts', import.meta.url), {
        type: 'module',
      });

      worker.onmessage = (event: MessageEvent<MarkdownWorkerResponse>) => {
        const { id, html } = event.data;
        const pending = pendingRef.current.get(id);
        if (pending) {
          pending.resolve(html);
          pendingRef.current.delete(id);
        }
      };

      worker.onerror = (error) => {
        // Reject all pending requests on worker error
        for (const [id, pending] of pendingRef.current.entries()) {
          pending.reject(new Error(`Worker error: ${error.message}`));
          pendingRef.current.delete(id);
        }
      };

      workerRef.current = worker;
    } catch {
      // Worker creation failed — fallback to sync mode
      workerRef.current = null;
    }

    return () => {
      workerRef.current?.terminate();
      workerRef.current = null;

      // Reject any still-pending requests
      for (const [id, pending] of pendingRef.current.entries()) {
        pending.reject(new Error('Worker terminated'));
        pendingRef.current.delete(id);
      }
    };
  }, []);

  const parseMarkdown = useCallback((content: string): Promise<string> => {
    const key = hashString(content);

    // Check cache first
    const cached = cacheRef.current.get(key);
    if (cached !== undefined) {
      return Promise.resolve(cached);
    }

    // If worker is not available, fall back to sync parsing
    if (!workerRef.current) {
      const html = syncParseMarkdown(content);
      cacheRef.current.set(key, html);
      return Promise.resolve(html);
    }

    // Dispatch to worker
    const id = `md_${(++idCounterRef.current).toString(36)}`;
    const request: MarkdownWorkerRequest = { id, content };

    return new Promise<string>((resolve, reject) => {
      pendingRef.current.set(id, {
        resolve: (html: string) => {
          cacheRef.current.set(key, html);
          resolve(html);
        },
        reject,
      });
      workerRef.current?.postMessage(request);
    });
  }, []);

  return { parseMarkdown };
}
