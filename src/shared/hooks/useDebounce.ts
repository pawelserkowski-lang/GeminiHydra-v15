/** Jaskier Shared Pattern */
import { useEffect, useRef, useState } from 'react';

/**
 * Debounce hook with leading-edge support (#36).
 * When `leading` is true, the first change fires immediately and subsequent
 * changes within the delay window are debounced.
 */
export function useDebounce<T>(value: T, delay: number = 300, leading: boolean = true): T {
  const [debouncedValue, setDebouncedValue] = useState(value);
  const isFirstRef = useRef(true);

  useEffect(() => {
    // Leading edge: fire immediately on first change
    if (leading && isFirstRef.current) {
      isFirstRef.current = false;
      setDebouncedValue(value);
      return;
    }

    const timer = setTimeout(() => {
      setDebouncedValue(value);
      isFirstRef.current = true; // Reset so next burst fires immediately
    }, delay);
    return () => clearTimeout(timer);
  }, [value, delay, leading]);

  return debouncedValue;
}
