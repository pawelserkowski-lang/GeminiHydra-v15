// src/features/chat/components/SearchOverlay.tsx
/**
 * GeminiHydra v15 - Message Search Overlay (#19)
 * ================================================
 * Ctrl+F search overlay for chat messages.
 * - Shows search input with match count ("3 of 12")
 * - Highlights matching text in messages via DOM mark injection
 * - Up/down arrows to navigate between matches
 * - Escape to close
 */

import { ChevronDown, ChevronUp, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import type { Message } from '@/stores/viewStore';

// ============================================================================
// TYPES
// ============================================================================

interface SearchOverlayProps {
  messages: Message[];
  /** Ref to the scroll container for scrolling to matches */
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  /** Whether the overlay is open */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
}

// ============================================================================
// HIGHLIGHT UTILITIES
// ============================================================================

/** Remove all existing search highlight marks and normalize text nodes. */
function clearHighlights(container: HTMLElement | null): void {
  if (!container) return;
  const marks = container.querySelectorAll('mark[data-search-highlight]');
  for (const mark of marks) {
    const parent = mark.parentNode;
    if (parent) {
      parent.replaceChild(document.createTextNode(mark.textContent ?? ''), mark);
      parent.normalize();
    }
  }
}

/** Count total query occurrences inside an element's text nodes. */
function countOccurrencesInElement(element: Element, lowerQuery: string): number {
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT, null);
  let count = 0;
  for (let node = walker.nextNode() as Text | null; node !== null; node = walker.nextNode() as Text | null) {
    const text = (node.textContent ?? '').toLowerCase();
    let idx = text.indexOf(lowerQuery, 0);
    while (idx !== -1) {
      count++;
      idx = text.indexOf(lowerQuery, idx + 1);
    }
  }
  return count;
}

/**
 * Walk text nodes inside `element`, wrap query matches in <mark> tags.
 * `counter.index` tracks the global match index for active-match styling.
 */
function highlightTextInElement(
  element: Element,
  lowerQuery: string,
  queryLength: number,
  activeMatchIndex: number,
  counter: { index: number },
): void {
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT, null);
  const textNodes: Text[] = [];
  for (let node = walker.nextNode() as Text | null; node !== null; node = walker.nextNode() as Text | null) {
    textNodes.push(node);
  }

  for (const textNode of textNodes) {
    const text = textNode.textContent ?? '';
    const lowerText = text.toLowerCase();
    const parts: { text: string; isMatch: boolean; isActive: boolean }[] = [];
    let lastEnd = 0;
    let searchFrom = 0;

    while (searchFrom < lowerText.length) {
      const idx = lowerText.indexOf(lowerQuery, searchFrom);
      if (idx === -1) break;

      if (idx > lastEnd) {
        parts.push({ text: text.slice(lastEnd, idx), isMatch: false, isActive: false });
      }
      parts.push({
        text: text.slice(idx, idx + queryLength),
        isMatch: true,
        isActive: counter.index === activeMatchIndex,
      });
      counter.index++;
      lastEnd = idx + queryLength;
      searchFrom = idx + 1;
    }

    if (parts.length === 0) continue;

    if (lastEnd < text.length) {
      parts.push({ text: text.slice(lastEnd), isMatch: false, isActive: false });
    }

    const fragment = document.createDocumentFragment();
    for (const part of parts) {
      if (part.isMatch) {
        const mark = document.createElement('mark');
        mark.setAttribute('data-search-highlight', 'true');
        mark.textContent = part.text;
        mark.style.borderRadius = '2px';
        mark.style.padding = '0 1px';
        if (part.isActive) {
          mark.style.backgroundColor = 'rgba(255, 165, 0, 0.6)';
          mark.style.color = 'white';
          mark.setAttribute('data-search-active', 'true');
        } else {
          mark.style.backgroundColor = 'rgba(255, 255, 0, 0.3)';
          mark.style.color = 'inherit';
        }
        fragment.appendChild(mark);
      } else {
        fragment.appendChild(document.createTextNode(part.text));
      }
    }

    textNode.parentNode?.replaceChild(fragment, textNode);
  }
}

/**
 * Applies highlight marks in the scroll container's `.markdown-body` elements.
 * First clears old marks, then injects new <mark> tags for each query occurrence.
 */
function applyHighlights(container: HTMLElement | null, query: string, activeMatchIndex: number): void {
  if (!container) return;

  clearHighlights(container);

  if (!query.trim()) return;

  const messageBodies = container.querySelectorAll('.markdown-body');
  const lowerQuery = query.toLowerCase();
  let globalMatchIndex = 0;

  for (const body of messageBodies) {
    const count = countOccurrencesInElement(body, lowerQuery);
    highlightTextInElement(body, lowerQuery, query.length, activeMatchIndex, { index: globalMatchIndex });
    globalMatchIndex += count;
  }
}

// ============================================================================
// COMPONENT
// ============================================================================

export const SearchOverlay = memo<SearchOverlayProps>(({ messages, scrollContainerRef, isOpen, onClose }) => {
  const theme = useViewTheme();
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);

  // Count total matches across all messages
  const totalMatches = useMemo(() => {
    if (!query.trim()) return 0;
    const lowerQuery = query.toLowerCase();
    let count = 0;
    for (const msg of messages) {
      const lowerContent = msg.content.toLowerCase();
      let idx = lowerContent.indexOf(lowerQuery, 0);
      while (idx !== -1) {
        count++;
        idx = lowerContent.indexOf(lowerQuery, idx + 1);
      }
    }
    return count;
  }, [messages, query]);

  // Clamp active index
  useEffect(() => {
    if (activeIndex >= totalMatches) {
      setActiveIndex(Math.max(0, totalMatches - 1));
    }
  }, [totalMatches, activeIndex]);

  // Apply DOM highlights
  useEffect(() => {
    if (!isOpen) {
      clearHighlights(scrollContainerRef.current);
      return;
    }
    applyHighlights(scrollContainerRef.current, query, activeIndex);
  }, [isOpen, query, activeIndex, scrollContainerRef]);

  // Scroll active match into view
  useEffect(() => {
    if (!isOpen || totalMatches === 0) return;
    const container = scrollContainerRef.current;
    if (!container) return;

    requestAnimationFrame(() => {
      const activeMark = container.querySelector('mark[data-search-active="true"]');
      if (activeMark) {
        activeMark.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    });
  }, [isOpen, totalMatches, scrollContainerRef]);

  // Focus input when opening
  useEffect(() => {
    if (isOpen) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  // Reset state when closing
  useEffect(() => {
    if (!isOpen) {
      setQuery('');
      setActiveIndex(0);
    }
  }, [isOpen]);

  // Clean up highlights on unmount
  useEffect(() => {
    return () => {
      clearHighlights(scrollContainerRef.current);
    };
  }, [scrollContainerRef]);

  const handlePrevious = useCallback(() => {
    if (totalMatches === 0) return;
    setActiveIndex((prev) => (prev <= 0 ? totalMatches - 1 : prev - 1));
  }, [totalMatches]);

  const handleNext = useCallback(() => {
    if (totalMatches === 0) return;
    setActiveIndex((prev) => (prev >= totalMatches - 1 ? 0 : prev + 1));
  }, [totalMatches]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      } else if (e.key === 'Enter') {
        if (e.shiftKey) {
          handlePrevious();
        } else {
          handleNext();
        }
      }
    },
    [onClose, handleNext, handlePrevious],
  );

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0, y: -20 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -20 }}
          transition={{ duration: 0.15, ease: 'easeOut' }}
          className={cn(
            'absolute top-2 right-3 z-30 flex items-center gap-2',
            'px-3 py-2 rounded-xl shadow-xl backdrop-blur-xl',
            theme.isLight ? 'bg-white/90 border border-slate-200/60' : 'bg-black/80 border border-white/15',
          )}
          onKeyDown={handleKeyDown}
        >
          <Search size={14} className={theme.iconMuted} />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActiveIndex(0);
            }}
            placeholder="Search messages..."
            className={cn(
              'w-40 sm:w-56 bg-transparent text-sm font-mono outline-none',
              'placeholder:opacity-40',
              theme.isLight ? 'text-black placeholder:text-gray-400' : 'text-white placeholder:text-white/30',
            )}
          />

          {/* Match count */}
          {query.trim() && (
            <span className={cn('text-xs font-mono whitespace-nowrap', theme.textMuted)}>
              {totalMatches === 0 ? 'No matches' : `${activeIndex + 1} of ${totalMatches}`}
            </span>
          )}

          {/* Navigation arrows */}
          <div className="flex items-center gap-0.5">
            <button
              type="button"
              onClick={handlePrevious}
              disabled={totalMatches === 0}
              className={cn(
                'p-1 rounded transition-colors',
                'disabled:opacity-30',
                theme.isLight ? 'hover:bg-black/5' : 'hover:bg-white/10',
              )}
              title="Previous match (Shift+Enter)"
            >
              <ChevronUp size={14} />
            </button>
            <button
              type="button"
              onClick={handleNext}
              disabled={totalMatches === 0}
              className={cn(
                'p-1 rounded transition-colors',
                'disabled:opacity-30',
                theme.isLight ? 'hover:bg-black/5' : 'hover:bg-white/10',
              )}
              title="Next match (Enter)"
            >
              <ChevronDown size={14} />
            </button>
          </div>

          {/* Close button */}
          <button
            type="button"
            onClick={onClose}
            className={cn('p-1 rounded transition-colors', theme.isLight ? 'hover:bg-black/5' : 'hover:bg-white/10')}
            title="Close (Escape)"
          >
            <X size={14} />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

SearchOverlay.displayName = 'SearchOverlay';

export default SearchOverlay;
