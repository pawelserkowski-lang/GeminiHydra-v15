// src/shared/hooks/useTextareaAutoResize.ts
/**
 * useTextareaAutoResize — Auto-resize textarea to fit content
 * =============================================================
 * Extracted from ChatInput.tsx for reusability.
 * Returns a stable `adjustHeight` callback that resizes the textarea
 * between minRows and maxRows based on scrollHeight.
 */

import { type RefObject, useCallback } from 'react';

interface UseTextareaAutoResizeOptions {
  /** Ref to the textarea element. */
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  /** Line height in pixels (default: 24). */
  lineHeight?: number;
  /** Minimum number of rows (default: 1). */
  minRows?: number;
  /** Maximum number of rows (default: 12). */
  maxRows?: number;
}

/**
 * Returns a stable `adjustHeight` function that resizes the textarea
 * to fit its content, clamped between minRows and maxRows.
 */
export function useTextareaAutoResize({
  textareaRef,
  lineHeight = 24,
  minRows = 1,
  maxRows = 12,
}: UseTextareaAutoResizeOptions): () => void {
  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    const maxHeight = lineHeight * maxRows;
    const minHeight = lineHeight * minRows;
    el.style.height = `${Math.min(Math.max(el.scrollHeight, minHeight), maxHeight)}px`;
  }, [textareaRef, lineHeight, minRows, maxRows]);

  return adjustHeight;
}
