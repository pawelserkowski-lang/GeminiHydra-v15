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

import { AlertCircle, ChevronDown, FolderOpen, Network, Send, StopCircle, X } from 'lucide-react';
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

import { useTranslation } from 'react-i18next';
import { Button } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { WorkingFolderPicker } from './WorkingFolderPicker';

// ============================================================================
// TYPES
// ============================================================================

export type OrchestrationMode = 'direct' | 'orchestrate';
export type OrchestrationPattern = 'auto' | 'sequential' | 'parallel' | 'loop' | 'hierarchical' | 'review' | 'security';

interface ChatInputProps {
  /** Whether the assistant is currently streaming a response. */
  isStreaming: boolean;
  /** Callback fired when the user submits a message. */
  onSubmit: (prompt: string, image: string | null) => void;
  /** Callback fired for orchestrated submissions. */
  onOrchestrate?: (prompt: string, pattern: string) => void;
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
  /** Attach a local file by path. */
  onAttachPath?: (path: string) => void;
  /** Previous user prompts for arrow-key navigation (newest last). */
  promptHistory?: string[];
  /** Per-session working directory props */
  sessionId?: string;
  workingDirectory?: string;
  onWorkingDirectoryChange?: (wd: string) => void;
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

const PATTERN_OPTIONS: Array<{ value: OrchestrationPattern; label: string }> = [
  { value: 'auto', label: 'Auto' },
  { value: 'hierarchical', label: 'Hierarchical' },
  { value: 'sequential', label: 'Sequential' },
  { value: 'parallel', label: 'Parallel' },
  { value: 'loop', label: 'Loop' },
  { value: 'review', label: 'Code Review' },
  { value: 'security', label: 'Security Review' },
];

export const ChatInput = memo<ChatInputProps>(
  ({
    isStreaming,
    onSubmit,
    onOrchestrate,
    onStop,
    pendingImage,
    onClearImage,
    onPasteImage,
    onPasteFile,
    promptHistory = [],
    sessionId,
    workingDirectory,
    onWorkingDirectoryChange,
  }) => {
    const { t } = useTranslation();
    const theme = useViewTheme();
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const [value, setValue] = useState('');
    const [error, setError] = useState<string | null>(null);

    // Orchestration mode state
    const [orchMode, setOrchMode] = useState<OrchestrationMode>('direct');
    const [orchPattern, setOrchPattern] = useState<OrchestrationPattern>('auto');
    const [showPatternPicker, setShowPatternPicker] = useState(false);

    // Prompt history navigation
    const [historyIndex, setHistoryIndex] = useState(-1);
    const savedDraftRef = useRef('');

    // Reset history index when session changes (global history persists across sessions)
    const prevSessionRef = useRef(sessionId);
    if (prevSessionRef.current !== sessionId) {
      prevSessionRef.current = sessionId;
      setHistoryIndex(-1);
      savedDraftRef.current = '';
    }

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
          setError(t('chat.messageTooLong'));
        } else {
          setError(null);
        }
        requestAnimationFrame(adjustHeight);
      },
      [adjustHeight, t],
    );

    // ----- Submit -------------------------------------------------------

    const handleSubmit = useCallback(() => {
      if (!canSubmit) return;
      const trimmed = value.trim();
      if (orchMode === 'orchestrate' && onOrchestrate) {
        const pattern = orchPattern === 'auto' ? 'hierarchical' : orchPattern;
        onOrchestrate(trimmed, pattern);
      } else {
        onSubmit(trimmed, pendingImage);
      }
      setValue('');
      setError(null);
      setShowPatternPicker(false);
      requestAnimationFrame(() => {
        if (textareaRef.current) {
          textareaRef.current.style.height = 'auto';
        }
      });
    }, [canSubmit, onSubmit, onOrchestrate, value, pendingImage, orchMode, orchPattern]);

    // ----- Key handling (Enter to send, Shift/Ctrl+Enter for newline) ----

    const handleKeyDown = useCallback(
      (e: KeyboardEvent<HTMLTextAreaElement>) => {
        if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
          e.preventDefault();
          handleSubmit();
          setHistoryIndex(-1);
          savedDraftRef.current = '';
        } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          const el = e.currentTarget;
          const { selectionStart, selectionEnd } = el;
          const newValue = `${value.substring(0, selectionStart)}\n${value.substring(selectionEnd)}`;
          setValue(newValue);
          requestAnimationFrame(() => {
            el.selectionStart = el.selectionEnd = selectionStart + 1;
            adjustHeight();
          });
        } else if (e.key === 'ArrowUp' && promptHistory.length > 0) {
          const el = e.currentTarget;
          const isAtStart = el.selectionStart === 0 && el.selectionEnd === 0;
          const isSingleLine = !value.includes('\n');
          if (isAtStart || (isSingleLine && historyIndex === -1)) {
            e.preventDefault();
            if (historyIndex === -1) {
              savedDraftRef.current = value;
            }
            const nextIndex = historyIndex === -1 ? promptHistory.length - 1 : Math.max(0, historyIndex - 1);
            setHistoryIndex(nextIndex);
            const historyValue = promptHistory[nextIndex] ?? '';
            setValue(historyValue);
            requestAnimationFrame(() => {
              if (textareaRef.current) {
                textareaRef.current.selectionStart = textareaRef.current.selectionEnd = historyValue.length;
              }
              adjustHeight();
            });
          }
        } else if (e.key === 'ArrowDown' && historyIndex >= 0) {
          const el = e.currentTarget;
          const isAtEnd = el.selectionStart === value.length;
          const isSingleLine = !value.includes('\n');
          if (isAtEnd || isSingleLine) {
            e.preventDefault();
            if (historyIndex >= promptHistory.length - 1) {
              setHistoryIndex(-1);
              setValue(savedDraftRef.current);
              requestAnimationFrame(adjustHeight);
            } else {
              const nextIndex = historyIndex + 1;
              setHistoryIndex(nextIndex);
              const historyValue = promptHistory[nextIndex] ?? '';
              setValue(historyValue);
              requestAnimationFrame(adjustHeight);
            }
          }
        }
      },
      [handleSubmit, value, adjustHeight, promptHistory, historyIndex],
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

    // ----- File picker handler -------------------------------------------

    const handleFileSelect = useCallback(
      (e: ChangeEvent<HTMLInputElement>) => {
        const files = e.target.files;
        if (!files || files.length === 0) return;

        for (const file of Array.from(files)) {
          if (file.type.startsWith('image/')) {
            const reader = new FileReader();
            reader.onload = (event) => {
              if (event.target?.result && typeof event.target.result === 'string') {
                onPasteImage?.(event.target.result);
              }
            };
            reader.readAsDataURL(file);
          } else if (file.size < 5 * 1024 * 1024) {
            const reader = new FileReader();
            reader.onload = (event) => {
              if (event.target?.result && typeof event.target.result === 'string') {
                onPasteFile?.(event.target.result.substring(0, 20_000), file.name);
              }
            };
            reader.readAsText(file);
          }
        }

        // Reset so the same files can be selected again
        e.target.value = '';
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
                'flex items-center gap-2 text-sm',
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

        {/* Per-session working folder picker */}
        {sessionId && onWorkingDirectoryChange && (
          <WorkingFolderPicker
            sessionId={sessionId}
            workingDirectory={workingDirectory ?? ''}
            onDirectoryChange={onWorkingDirectoryChange}
          />
        )}

        <div className="flex gap-3 items-end w-full">
          {/* Textarea wrapper */}
          <div className="relative flex-1 group">
            <textarea
              ref={textareaRef}
              data-testid="chat-textarea"
              aria-label={t('chat.messageInput', 'Type your message')}
              value={value}
              onChange={handleChange}
              onKeyDown={handleKeyDown}
              onPaste={handlePaste}
              disabled={isStreaming}
              rows={MIN_ROWS}
              placeholder={pendingImage ? t('chat.describeVisualContext') : t('chat.typeMessage')}
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
                    'text-sm font-mono transition-colors duration-300',
                    isOverLimit ? 'text-red-500 font-bold' : 'text-[var(--matrix-text-dim)] opacity-50',
                  )}
                >
                  {charCount}/{MAX_CHARS}
                </div>
              )}
            </div>
          </div>

          {/* Orchestration mode toggle */}
          {onOrchestrate && (
            <div className="relative">
              <Button
                type="button"
                variant={orchMode === 'orchestrate' ? 'primary' : 'ghost'}
                size="md"
                onClick={() => {
                  if (orchMode === 'direct') {
                    setOrchMode('orchestrate');
                    setShowPatternPicker(true);
                  } else {
                    setOrchMode('direct');
                    setShowPatternPicker(false);
                  }
                }}
                className="mb-[1px]"
                title={
                  orchMode === 'orchestrate'
                    ? t('chat.switchToDirect', 'Switch to direct mode')
                    : t('chat.switchToOrchestrate', 'Switch to orchestrate mode')
                }
                data-testid="btn-orchestrate-toggle"
              >
                <Network size={18} />
                <ChevronDown size={12} className="ml-0.5" />
              </Button>
              {/* Pattern picker dropdown */}
              <AnimatePresence>
                {showPatternPicker && orchMode === 'orchestrate' && (
                  <motion.div
                    initial={{ opacity: 0, y: 4, scale: 0.95 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: 4, scale: 0.95 }}
                    className={cn(
                      'absolute bottom-full right-0 mb-2 z-50',
                      'min-w-[160px] py-1 rounded-lg shadow-lg',
                      'border border-white/10',
                      theme.dropdown,
                    )}
                  >
                    {PATTERN_OPTIONS.map((opt) => (
                      <button
                        key={opt.value}
                        type="button"
                        onClick={() => {
                          setOrchPattern(opt.value);
                          setShowPatternPicker(false);
                        }}
                        className={cn(
                          'w-full text-left px-3 py-1.5 text-sm font-mono transition-colors',
                          theme.dropdownItem,
                          orchPattern === opt.value && 'font-bold',
                        )}
                      >
                        {orchPattern === opt.value && '> '}
                        {opt.label}
                      </button>
                    ))}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          )}

          {/* Attach file via native picker */}
          {(onPasteImage || onPasteFile) && (
            <>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                className="hidden"
                onChange={handleFileSelect}
                accept="image/*,.txt,.md,.ts,.tsx,.js,.jsx,.json,.css,.html,.py,.rs,.toml,.yaml,.yml,.xml,.csv,.log,.sh,.bat,.sql,.env"
              />
              <Button
                type="button"
                variant="ghost"
                size="md"
                onClick={() => fileInputRef.current?.click()}
                className="mb-[1px]"
                title={t('chat.attachLocalFile', 'Attach local file')}
                data-testid="btn-attach-file"
              >
                <FolderOpen size={20} />
              </Button>
            </>
          )}

          {/* Send / Stop button */}
          {isStreaming ? (
            <Button
              type="button"
              variant="danger"
              size="md"
              onClick={onStop}
              className="mb-[1px]"
              title={t('chat.stopGeneration', 'Stop generation')}
              aria-label={t('chat.stopGeneration', 'Stop generation')}
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
              title={t('chat.send', 'Send (Enter)')}
              aria-label={t('chat.send', 'Send message')}
              data-testid="btn-send"
            >
              <Send size={20} strokeWidth={2.5} className="ml-0.5" />
            </Button>
          )}
        </div>
      </form>
    );
  },
);

ChatInput.displayName = 'ChatInput';

export default ChatInput;
