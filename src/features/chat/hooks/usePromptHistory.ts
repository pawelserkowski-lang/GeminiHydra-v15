/**
 * usePromptHistory — Persistent global prompt history (SQL-backed).
 * Jaskier Shared Pattern — identical in GH + CH.
 *
 * Stores all user prompts across sessions in the backend DB.
 * localStorage cache provides instant startup before API response arrives.
 * ArrowUp/Down in ChatInput cycles through global history.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { apiGet, apiPost } from '@/shared/api/client';

const CACHE_KEY = 'prompt-history-cache';

export function usePromptHistory() {
  const [history, setHistory] = useState<string[]>(() => {
    try {
      const cached = localStorage.getItem(CACHE_KEY);
      return cached ? (JSON.parse(cached) as string[]) : [];
    } catch {
      return [];
    }
  });
  const fetchedRef = useRef(false);

  // Fetch from DB on mount (once)
  useEffect(() => {
    if (fetchedRef.current) return;
    fetchedRef.current = true;
    apiGet<string[]>('/api/prompt-history')
      .then((data) => {
        setHistory(data);
        try {
          localStorage.setItem(CACHE_KEY, JSON.stringify(data));
        } catch {
          // localStorage full — ignore
        }
      })
      .catch(() => {
        // keep localStorage cache on error
      });
  }, []);

  const addPrompt = useCallback((content: string) => {
    const trimmed = content.trim();
    if (!trimmed) return;
    // Optimistic update + consecutive dedup
    setHistory((prev) => {
      if (prev.length > 0 && prev[prev.length - 1] === trimmed) return prev;
      const next = [...prev, trimmed];
      try {
        localStorage.setItem(CACHE_KEY, JSON.stringify(next));
      } catch {
        // localStorage full — ignore
      }
      return next;
    });
    // Fire-and-forget POST to backend
    apiPost('/api/prompt-history', { content: trimmed }).catch(() => {});
  }, []);

  return { promptHistory: history, addPrompt };
}
