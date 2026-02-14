// src/features/chat/components/ChatInput.tsx
/**
 * GeminiHydra v15 - ChatInput
 * ============================
 * Auto-resizing textarea with send button, character counter,
 * image preview, paste/drop handling, and model selector.
 *
 * Ported from legacy ChatInput.tsx with:
 * - useViewTheme glassmorphism
 * - motion animations
 * - Atom/Molecule reuse (Button, ModelSelector)
 */

import { AlertCircle, Paperclip, Send, StopCircle, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import {
  type ChangeEvent,
  type ClipboardEvent,
  type KeyboardEvent,
  memo,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';

import { Button } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

export interface ChatInputProps {
  /** Whether the assistant is currently streaming a response. */
  isStreaming: boolean;
  /** Callback fired when the user submits a message. */
  onSubmit: (prompt: string, image: string | null) => void;
  /** Callback fired to stop an active stream. */
  onStop?: () => void;
  /** Base64 pending image (set externally via drag-drop). */
  pendingImage: string | null;
  /** Clear the pending image. */
  onClearImage: () => void;
  /** Handle pasted image from clipboard. */
  onPasteImage?: (base64: string) => void;
  /** Handle pasted text file from clipboard. */
  onPasteFile?: (content: string, filename: string) => void;
}

// ============================================================================
// CONSTANTS
// ============================================================================

const MAX_CHARS = 4000;
const MAX_ROWS = 12;
const MIN_ROWS = 1;

// ============================================================================
// IMAGE PREVIEW
// ============================================================================

interface ImagePreviewProps {
  src: string;
  onRemove: () => void;
}

const ImagePreview = memo<ImagePreviewProps>(({ src, onRemove }) => (
  <motion.div
    layout
    initial={{ opacity: 0, scale: 0.8, y: 10 }}
    animate={{ opacity: 1, scale: 1, y: 0 }}
    exit={{ opacity: 0, scale: 0.8, y: 10 }}
    className="relative inline-block w-fit mb-3 group"
  >
    <img
      src={src}
      alt="Preview"
      className={cn(
        'h-24 w-auto rounded-xl border shadow-lg',
        'border-[var(--matrix-accent)]/50',
        'shadow-[0_0_15px_rgba(255,255,255,0.1)]',
      )}
    />
    <button
      type="button"
      onClick={onRemove}
      className={cn(
        'absolute -top-2 -right-2 p-1 rounded-full',
        'bg-red-500 text-white',
        'opacity-0 group-hover:opacity-100',
        'transition-all shadow-sm hover:scale-110',
      )}
    >
      <X size={14} strokeWidth={3} />
    </button>
  </motion.div>
));

ImagePreview.displayName = 'ImagePreview';

// ============================================================================
// CHAT INPUT COMPONENT
// ============================================================================

export const ChatInput = memo<ChatInputProps>(
  ({ isStreaming, onSubmit, onStop, pendingImage, onClearImage, onPasteImage, onPasteFile }) => {
    const theme = useViewTheme();
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const [value, setValue] = useState('');
    const [error, setError] = useState<string | null>(null);

    const charCount = value.length;
    const isOverLimit = charCount > MAX_CHARS;
    const canSubmit = !isStreaming && !isOverLimit && (value.trim().length > 0 || !!pendingImage);

    // ----- Auto-focus on mount ------------------------------------------

    useEffect(() => {
      textareaRef.current?.focus();
    }, []);

    // ----- Auto-resize textarea -----------------------------------------

    const adjustHeight = useCallback(() => {
      const el = textareaRef.current;
      if (!el) return;
      el.style.height = 'auto';
      const lineHeight = 24;
      const maxHeight = lineHeight * MAX_ROWS;
      const minHeight = lineHeight * MIN_ROWS;
      el.style.height = `${Math.min(Math.max(el.scrollHeight, minHeight), maxHeight)}px`;
    }, []);

    const handleChange = useCallback(
      (e: ChangeEvent<HTMLTextAreaElement>) => {
        const next = e.target.value;
        setValue(next);
        if (next.length > MAX_CHARS) {
          setError('Message too long');
        } else {
          setError(null);
        }
        requestAnimationFrame(adjustHeight);
      },
      [adjustHeight],
    );

    // ----- Submit -------------------------------------------------------

    const handleSubmit = useCallback(() => {
      if (!canSubmit) return;
      onSubmit(value.trim(), pendingImage);
      setValue('');
      setError(null);
      requestAnimationFrame(() => {
        if (textareaRef.current) {
          textareaRef.current.style.height = 'auto';
        }
      });
    }, [canSubmit, onSubmit, value, pendingImage]);

    // ----- Key handling (Enter to send, Shift+Enter for newline) --------

    const handleKeyDown = useCallback(
      (e: KeyboardEvent<HTMLTextAreaElement>) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          handleSubmit();
        }
      },
      [handleSubmit],
    );

    // ----- Paste handling -----------------------------------------------

    const handlePaste = useCallback(
      (e: ClipboardEvent<HTMLTextAreaElement>) => {
        const items = e.clipboardData.items;
        for (const item of items) {
          if (item.type.startsWith('image/')) {
            const blob = item.getAsFile();
            if (blob) {
              const reader = new FileReader();
              reader.onload = (event) => {
                if (event.target?.result && typeof event.target.result === 'string') {
                  onPasteImage?.(event.target.result);
                }
              };
              reader.readAsDataURL(blob);
              e.preventDefault();
              return;
            }
          }
          if (item.kind === 'file' && !item.type.startsWith('image/')) {
            const file = item.getAsFile();
            if (file && file.size < 5 * 1024 * 1024) {
              const reader = new FileReader();
              reader.onload = (event) => {
                if (event.target?.result && typeof event.target.result === 'string') {
                  onPasteFile?.(event.target.result.substring(0, 20_000), file.name);
                }
              };
              reader.readAsText(file);
              e.preventDefault();
              return;
            }
          }
        }
      },
      [onPasteImage, onPasteFile],
    );

    // ----- Render -------------------------------------------------------

    return (
      <form
        onSubmit={(e) => {
          e.preventDefault();
          handleSubmit();
        }}
        className="p-4 flex flex-col relative transition-all duration-300 z-10"
      >
        {/* Error toast */}
        <AnimatePresence>
          {error && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 5 }}
              className={cn(
                'absolute bottom-full left-4 mb-2',
                'flex items-center gap-2 text-xs',
                'text-red-400 bg-red-950/90 border border-red-500/30',
                'px-3 py-2 rounded-lg shadow-lg backdrop-blur-sm',
              )}
            >
              <AlertCircle size={14} />
              <span>{error}</span>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Image preview area */}
        <AnimatePresence>
          {pendingImage && (
            <div className="flex w-full px-2">
              <ImagePreview src={pendingImage} onRemove={onClearImage} />
            </div>
          )}
        </AnimatePresence>

        <div className="flex gap-3 items-end w-full">
          {/* Textarea wrapper */}
          <div className="relative flex-1 group">
            <textarea
              ref={textareaRef}
              data-testid="chat-textarea"
              value={value}
              onChange={handleChange}
              onKeyDown={handleKeyDown}
              onPaste={handlePaste}
              disabled={isStreaming}
              rows={MIN_ROWS}
              placeholder={pendingImage ? 'Describe the visual context...' : 'Type a message...'}
              className={cn(
                'w-full rounded-xl px-5 py-3 pr-24 resize-none',
                'font-mono text-base leading-6',
                'transition-all duration-300 shadow-inner',
                'focus:outline-none focus:ring-2 focus:ring-[var(--matrix-accent)]/50',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'scrollbar-hide',
                theme.input,
                isOverLimit && 'border-red-500 focus:ring-red-500',
                error && 'border-red-500/50',
              )}
            />

            {/* Focus glow effect */}
            <div className="absolute inset-0 rounded-xl bg-[var(--matrix-accent)]/5 opacity-0 group-focus-within:opacity-100 pointer-events-none transition-opacity duration-500 blur-sm" />

            {/* Character counter */}
            <div className="absolute right-3 bottom-2.5 flex items-center gap-3">
              {charCount > 0 && (
                <div
                  className={cn(
                    'text-xs font-mono transition-colors duration-300',
                    isOverLimit ? 'text-red-500 font-bold' : 'text-[var(--matrix-text-dim)] opacity-50',
                  )}
                >
                  {charCount}/{MAX_CHARS}
                </div>
              )}
            </div>
          </div>

          {/* Send / Stop button */}
          {isStreaming ? (
            <Button
              type="button"
              variant="danger"
              size="md"
              onClick={onStop}
              className="mb-[1px]"
              title="Stop generation"
              data-testid="btn-stop"
            >
              <StopCircle size={20} className="animate-pulse" />
            </Button>
          ) : (
            <Button
              type="submit"
              variant="primary"
              size="md"
              disabled={!canSubmit}
              className="mb-[1px]"
              title="Send (Enter)"
              data-testid="btn-send"
            >
              <Send size={20} strokeWidth={2.5} className="ml-0.5" />
            </Button>
          )}
        </div>

        {/* Footer hints */}
        <div className="flex justify-between px-2 mt-2">
          <span className={cn('text-xs flex items-center gap-1 opacity-50', theme.textMuted)}>
            <Paperclip size={10} />
            Ctrl+V: paste image or file
          </span>
          <span className={cn('text-xs font-mono opacity-50', theme.textMuted)}>Shift+Enter: new line</span>
        </div>
      </form>
    );
  },
);

ChatInput.displayName = 'ChatInput';

export default ChatInput;
