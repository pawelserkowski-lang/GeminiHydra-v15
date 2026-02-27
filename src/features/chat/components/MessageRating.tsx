// src/features/chat/components/MessageRating.tsx
/**
 * GeminiHydra v15 - MessageRating
 * ================================
 * Compact star-rating widget shown below assistant messages.
 * Supports hover preview, click-to-rate, and optional text feedback.
 * Sends rating to backend via POST /api/ratings.
 */

import { Star } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { apiPost } from '@/shared/api/client';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

interface MessageRatingProps {
  messageId: string;
  sessionId: string;
}

interface RatingPayload {
  message_id: string;
  session_id: string;
  rating: number;
  feedback?: string;
}

// ============================================================================
// COMPONENT
// ============================================================================

export const MessageRating = memo<MessageRatingProps>(({ messageId, sessionId }) => {
  const { t } = useTranslation();
  const [rating, setRating] = useState<number>(0);
  const [hovered, setHovered] = useState<number>(0);
  const [submitted, setSubmitted] = useState(false);
  const [showFeedback, setShowFeedback] = useState(false);
  const [feedback, setFeedback] = useState('');
  const [submitting, setSubmitting] = useState(false);

  // ----- Submit rating to backend ------------------------------------------

  const submitRating = useCallback(
    async (stars: number, feedbackText?: string) => {
      setSubmitting(true);
      try {
        const payload: RatingPayload = {
          message_id: messageId,
          session_id: sessionId,
          rating: stars,
          ...(feedbackText && { feedback: feedbackText }),
        };
        await apiPost('/api/ratings', payload);
        setSubmitted(true);
      } catch (err) {
        console.error('[MessageRating] Failed to submit rating:', err);
      } finally {
        setSubmitting(false);
      }
    },
    [messageId, sessionId],
  );

  // ----- Star click handler ------------------------------------------------

  const handleStarClick = useCallback(
    (star: number) => {
      if (submitted) return;
      setRating(star);
      setShowFeedback(true);
      // Submit immediately (feedback can be added later)
      void submitRating(star);
    },
    [submitted, submitRating],
  );

  // ----- Feedback submit ---------------------------------------------------

  const handleFeedbackSubmit = useCallback(() => {
    if (!feedback.trim() || !rating) return;
    // Re-submit with feedback text
    void submitRating(rating, feedback.trim());
    setShowFeedback(false);
  }, [feedback, rating, submitRating]);

  // ----- Already submitted — show static stars -----------------------------

  if (submitted && !showFeedback) {
    return (
      <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="flex items-center gap-1 mt-1.5">
        {[1, 2, 3, 4, 5].map((star) => (
          <Star
            key={star}
            size={13}
            className={cn(
              star <= rating
                ? 'fill-[var(--matrix-accent)] text-[var(--matrix-accent)]'
                : 'text-[var(--matrix-text-primary)]/20',
            )}
          />
        ))}
        <span className="text-[10px] ml-1 text-[var(--matrix-text-primary)]/40 font-mono">
          {t('chat.rated', 'Rated')}
        </span>
      </motion.div>
    );
  }

  // ----- Interactive stars -------------------------------------------------

  return (
    <div className="mt-1.5">
      <div className="flex items-center gap-0.5">
        {[1, 2, 3, 4, 5].map((star) => {
          const isActive = star <= (hovered || rating);
          return (
            <motion.button
              key={star}
              type="button"
              whileHover={{ scale: 1.2 }}
              whileTap={{ scale: 0.9 }}
              onMouseEnter={() => !submitted && setHovered(star)}
              onMouseLeave={() => setHovered(0)}
              onClick={() => handleStarClick(star)}
              disabled={submitting}
              className={cn(
                'p-0.5 rounded transition-colors duration-150',
                'focus:outline-none focus-visible:ring-1 focus-visible:ring-[var(--matrix-accent)]',
                submitting && 'opacity-50 cursor-not-allowed',
              )}
              aria-label={t('chat.rateStar', 'Rate {{star}} stars', { star })}
            >
              <Star
                size={13}
                className={cn(
                  'transition-colors duration-150',
                  isActive
                    ? 'fill-[var(--matrix-accent)] text-[var(--matrix-accent)]'
                    : 'text-[var(--matrix-text-primary)]/25 hover:text-[var(--matrix-text-primary)]/50',
                )}
              />
            </motion.button>
          );
        })}

        {!rating && (
          <span className="text-[10px] ml-1 text-[var(--matrix-text-primary)]/30 font-mono select-none">
            {t('chat.rateResponse', 'Rate')}
          </span>
        )}
      </div>

      {/* Optional feedback input — shown after rating */}
      <AnimatePresence>
        {showFeedback && rating > 0 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.2 }}
            className="overflow-hidden"
          >
            <div className="flex items-center gap-1.5 mt-1.5">
              <input
                type="text"
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleFeedbackSubmit()}
                placeholder={t('chat.feedbackPlaceholder', 'Optional feedback...')}
                className={cn(
                  'flex-1 text-xs font-mono px-2 py-1 rounded-md',
                  'bg-[var(--matrix-bg-primary)]/50 border border-[var(--matrix-accent)]/15',
                  'text-[var(--matrix-text-primary)] placeholder:text-[var(--matrix-text-primary)]/30',
                  'focus:outline-none focus:border-[var(--matrix-accent)]/40',
                  'transition-colors duration-150',
                )}
                maxLength={500}
                disabled={submitting}
              />
              <motion.button
                type="button"
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={handleFeedbackSubmit}
                disabled={!feedback.trim() || submitting}
                className={cn(
                  'text-[10px] font-mono px-2 py-1 rounded-md',
                  'bg-[var(--matrix-accent)]/15 text-[var(--matrix-accent)]',
                  'hover:bg-[var(--matrix-accent)]/25',
                  'disabled:opacity-30 disabled:cursor-not-allowed',
                  'transition-colors duration-150',
                )}
              >
                {t('chat.send', 'Send')}
              </motion.button>
              <motion.button
                type="button"
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={() => setShowFeedback(false)}
                className={cn(
                  'text-[10px] font-mono px-1.5 py-1 rounded-md',
                  'text-[var(--matrix-text-primary)]/40',
                  'hover:text-[var(--matrix-text-primary)]/60',
                  'transition-colors duration-150',
                )}
              >
                {t('chat.skip', 'Skip')}
              </motion.button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

MessageRating.displayName = 'MessageRating';

export default MessageRating;
