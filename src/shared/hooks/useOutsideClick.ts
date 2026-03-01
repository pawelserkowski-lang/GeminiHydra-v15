/** Jaskier Shared Pattern */
import { type RefObject, useEffect } from 'react';

/**
 * Calls `onOutsideClick` when a mousedown event occurs outside the element
 * referenced by `ref`.
 */
export function useOutsideClick(ref: RefObject<HTMLElement | null>, onOutsideClick: () => void): void {
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onOutsideClick();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [ref, onOutsideClick]);
}
