// src/shared/hooks/useFocusTrap.ts
/** Jaskier Shared Pattern */
/**
 * Focus Trap Hook
 * ================
 * Traps keyboard focus within a container element (typically a modal).
 * - Tab / Shift+Tab cycles through focusable elements
 * - Escape key calls onEscape callback
 * - Auto-focuses the first focusable element on mount
 * - Restores focus to the previously focused element on unmount
 */

import { type RefObject, useEffect, useRef } from 'react';

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

interface UseFocusTrapOptions {
  /** Whether the trap is currently active */
  active: boolean;
  /** Called when Escape is pressed */
  onEscape?: () => void;
}

export function useFocusTrap<T extends HTMLElement>(ref: RefObject<T | null>, options: UseFocusTrapOptions): void {
  const { active, onEscape } = options;
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!active) return;

    // Store the currently focused element to restore later
    previousFocusRef.current = document.activeElement as HTMLElement | null;

    const container = ref.current;
    if (!container) return;

    // Focus the first focusable element after a tick (allows render to complete)
    const timer = setTimeout(() => {
      const focusable = container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
      const firstEl = focusable[0];
      if (firstEl) {
        firstEl.focus();
      }
    }, 50);

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onEscape?.();
        return;
      }

      if (e.key !== 'Tab') return;

      const focusableElements = container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR);
      if (focusableElements.length === 0) return;

      const first = focusableElements[0] as HTMLElement;
      const last = focusableElements[focusableElements.length - 1] as HTMLElement;

      if (e.shiftKey) {
        // Shift+Tab: wrap from first to last
        if (document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        // Tab: wrap from last to first
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    container.addEventListener('keydown', handleKeyDown);

    return () => {
      clearTimeout(timer);
      container.removeEventListener('keydown', handleKeyDown);

      // Restore focus to the previously focused element
      if (previousFocusRef.current && typeof previousFocusRef.current.focus === 'function') {
        previousFocusRef.current.focus();
      }
    };
  }, [active, ref, onEscape]);
}
