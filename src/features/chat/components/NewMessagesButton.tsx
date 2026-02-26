// src/features/chat/components/NewMessagesButton.tsx
/**
 * GeminiHydra v15 - New Messages Button (#20)
 * ==============================================
 * Floating pill button shown when user scrolls up and new messages arrive.
 * Uses IntersectionObserver to detect if the scroll anchor is visible.
 * Click scrolls to the bottom. Auto-hides when user scrolls back down.
 */

import { ArrowDown } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

interface NewMessagesButtonProps {
  /** Ref to the scroll anchor element at the bottom of messages */
  scrollAnchorRef: React.RefObject<HTMLDivElement | null>;
  /** Ref to the scroll container */
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  /** Current message count — used to detect new messages */
  messageCount: number;
}

// ============================================================================
// COMPONENT
// ============================================================================

export const NewMessagesButton = memo<NewMessagesButtonProps>(
  ({ scrollAnchorRef, scrollContainerRef, messageCount }) => {
    const { t } = useTranslation();
    const theme = useViewTheme();
    const [isAtBottom, setIsAtBottom] = useState(true);
    const [hasNewMessages, setHasNewMessages] = useState(false);
    const prevMessageCountRef = useRef(messageCount);

    // Track whether the scroll anchor is visible via IntersectionObserver
    useEffect(() => {
      const anchor = scrollAnchorRef.current;
      const container = scrollContainerRef.current;
      if (!anchor || !container) return;

      const observer = new IntersectionObserver(
        ([entry]) => {
          const visible = entry?.isIntersecting ?? false;
          setIsAtBottom(visible);
          if (visible) {
            setHasNewMessages(false);
          }
        },
        {
          root: container,
          // Trigger when anchor is within 50px of the viewport bottom
          rootMargin: '0px 0px 50px 0px',
          threshold: 0,
        },
      );

      observer.observe(anchor);
      return () => observer.disconnect();
    }, [scrollAnchorRef, scrollContainerRef]);

    // Detect new messages arriving while scrolled up
    useEffect(() => {
      if (messageCount > prevMessageCountRef.current && !isAtBottom) {
        setHasNewMessages(true);
      }
      prevMessageCountRef.current = messageCount;
    }, [messageCount, isAtBottom]);

    const handleScrollToBottom = useCallback(() => {
      scrollAnchorRef.current?.scrollIntoView({ behavior: 'smooth' });
      setHasNewMessages(false);
    }, [scrollAnchorRef]);

    const showButton = hasNewMessages && !isAtBottom;

    return (
      <AnimatePresence>
        {showButton && (
          <motion.button
            type="button"
            initial={{ opacity: 0, y: 10, scale: 0.9 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: 0.9 }}
            transition={{ duration: 0.2, ease: 'easeOut' }}
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
            onClick={handleScrollToBottom}
            className={cn(
              'absolute bottom-4 left-1/2 -translate-x-1/2 z-20',
              'flex items-center gap-1.5 px-4 py-2 rounded-full',
              'text-xs font-mono shadow-lg backdrop-blur-xl',
              'cursor-pointer',
              theme.isLight
                ? 'bg-emerald-500/90 text-white border border-emerald-400/50 hover:bg-emerald-500'
                : 'bg-white/15 text-white border border-white/20 hover:bg-white/25',
            )}
            title={t('chat.scrollToBottom', 'Scroll to latest messages')}
          >
            <span>{t('chat.newMessages', 'New messages')}</span>
            <ArrowDown size={12} />
          </motion.button>
        )}
      </AnimatePresence>
    );
  },
);

NewMessagesButton.displayName = 'NewMessagesButton';

export default NewMessagesButton;
