// src/features/chat/components/ChatContainer.tsx
/**
 * GeminiHydra v15 - ChatContainer
 * =================================
 * Main chat interface: scrollable message list with auto-scroll,
 * drag-drop file zone, context menu, empty state, streaming indicator,
 * and ChatInput at the bottom.
 *
 * Ported from legacy ChatContainer.tsx + MessageList.tsx with:
 * - viewStore integration for session/message management
 * - Glassmorphism via useViewTheme
 * - motion animations
 */

import { Copy, FileText, Paperclip, Sparkles, Trash2, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import {
  type DragEvent,
  memo,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';
import { toast } from 'sonner';

import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type Message, selectCurrentMessages, useViewStore } from '@/stores/viewStore';

import { ChatInput } from './ChatInput';
import { MessageBubble } from './MessageBubble';

// ============================================================================
// TYPES
// ============================================================================

export interface ChatContainerProps {
  /** Whether the assistant is currently streaming. */
  isStreaming: boolean;
  /** Callback to submit a new message. */
  onSubmit: (prompt: string, image: string | null) => void;
  /** Callback to stop active stream. */
  onStop?: () => void;
}

// ============================================================================
// CONTEXT MENU STATE
// ============================================================================

interface ContextMenuState {
  x: number;
  y: number;
  message: Message;
}

// ============================================================================
// DRAG-DROP ZONE
// ============================================================================

interface DragDropZoneProps {
  children: ReactNode;
  onImageDrop: (base64: string) => void;
  onTextDrop: (content: string, filename: string) => void;
}

const DragDropZone = memo<DragDropZoneProps>(({ children, onImageDrop, onTextDrop }) => {
  const [isDragActive, setIsDragActive] = useState(false);

  const handleDrag = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setIsDragActive(true);
    } else if (e.type === 'dragleave') {
      setIsDragActive(false);
    }
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragActive(false);

      const file = e.dataTransfer.files[0];
      if (!file) return;

      const MAX_SIZE = 5 * 1024 * 1024;
      if (file.size > MAX_SIZE) {
        toast.error('File too large (max 5MB)');
        return;
      }

      const reader = new FileReader();
      if (file.type.startsWith('image/')) {
        reader.onload = (event) => {
          const result = event.target?.result;
          if (typeof result === 'string') onImageDrop(result);
        };
        reader.readAsDataURL(file);
      } else {
        reader.onload = (event) => {
          const result = event.target?.result;
          if (typeof result === 'string') {
            onTextDrop(result.substring(0, 20_000), file.name);
          }
        };
        reader.readAsText(file);
      }
    },
    [onImageDrop, onTextDrop],
  );

  return (
    <section
      aria-label="File drop zone"
      className="flex flex-col w-full h-full min-h-0 relative"
      onDragEnter={handleDrag}
      onDragLeave={handleDrag}
      onDragOver={handleDrag}
      onDrop={handleDrop}
    >
      {/* Drop overlay */}
      <AnimatePresence>
        {isDragActive && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className={cn(
              'absolute inset-0 z-50',
              'bg-black/80 backdrop-blur-sm',
              'flex items-center justify-center',
              'border-4 border-[var(--matrix-accent)] border-dashed rounded-xl',
              'pointer-events-none',
            )}
          >
            <div className="text-[var(--matrix-accent)] text-2xl font-mono animate-pulse flex flex-col items-center gap-4">
              <Paperclip size={64} />
              <span>DROP FILE TO ADD CONTEXT</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      {children}
    </section>
  );
});

DragDropZone.displayName = 'DragDropZone';

// ============================================================================
// CONTEXT MENU
// ============================================================================

interface ChatContextMenuProps {
  x: number;
  y: number;
  isUser: boolean;
  onClose: () => void;
  onCopy: () => void;
  onDelete?: () => void;
}

const ChatContextMenu = memo<ChatContextMenuProps>(({ x, y, isUser: _isUser, onClose, onCopy, onDelete }) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const theme = useViewTheme();

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      role="menu"
      className={cn('fixed z-50 min-w-[150px] py-1 text-sm', 'rounded-lg shadow-lg', theme.dropdown)}
      style={{ top: y, left: x }}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
      }}
    >
      <button
        type="button"
        onClick={onCopy}
        className={cn('w-full text-left px-4 py-2 flex items-center gap-2 transition-colors', theme.dropdownItem)}
      >
        <Copy size={14} />
        <span>Copy</span>
      </button>

      {onDelete && (
        <>
          <div className={theme.divider} />
          <button
            type="button"
            onClick={onDelete}
            className="w-full text-left px-4 py-2 hover:bg-red-900/30 text-red-400 flex items-center gap-2 transition-colors rounded-lg"
          >
            <Trash2 size={14} />
            <span>Delete</span>
          </button>
        </>
      )}

      <div className={theme.divider} />
      <button
        type="button"
        onClick={onClose}
        className={cn(
          'w-full text-left px-4 py-2 flex items-center gap-2 transition-colors',
          theme.dropdownItem,
          'opacity-60',
        )}
      >
        <X size={14} />
        <span>Cancel</span>
      </button>
    </div>
  );
});

ChatContextMenu.displayName = 'ChatContextMenu';

// ============================================================================
// EMPTY STATE
// ============================================================================

const EmptyState = memo(() => {
  const theme = useViewTheme();
  return (
    <div className="h-full flex flex-col items-center justify-center gap-3">
      <Sparkles size={48} className={cn(theme.iconMuted, 'opacity-30')} />
      <p className={cn('text-sm font-mono', theme.textMuted)}>Type a message to start a conversation...</p>
    </div>
  );
});

EmptyState.displayName = 'EmptyState';

// ============================================================================
// STREAMING INDICATOR
// ============================================================================

const StreamingIndicator = memo(() => {
  const theme = useViewTheme();
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="flex items-center gap-2 px-4 py-2"
    >
      <div className="flex gap-1">
        {[0, 1, 2].map((i) => (
          <motion.span
            key={i}
            className={cn('w-1.5 h-1.5 rounded-full', theme.accentBg)}
            animate={{ opacity: [0.3, 1, 0.3] }}
            transition={{
              duration: 1.2,
              repeat: Number.POSITIVE_INFINITY,
              delay: i * 0.2,
            }}
          />
        ))}
      </div>
      <span className={cn('text-xs font-mono', theme.textMuted)}>Generating...</span>
    </motion.div>
  );
});

StreamingIndicator.displayName = 'StreamingIndicator';

// ============================================================================
// CHAT CONTAINER
// ============================================================================

export const ChatContainer = memo<ChatContainerProps>(({ isStreaming, onSubmit, onStop }) => {
  const theme = useViewTheme();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Store
  const messages = useViewStore(selectCurrentMessages);

  // Local state
  const [pendingImage, setPendingImage] = useState<string | null>(null);
  const [textContext, setTextContext] = useState('');
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  // ----- Auto-scroll to bottom ----------------------------------------

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional scroll trigger on message changes
  useEffect(() => {
    scrollToBottom();
  }, [messages, scrollToBottom]);

  // ----- Image drop/paste handlers ------------------------------------

  const handleImageDrop = useCallback((base64: string) => {
    const MAX_IMAGE_SIZE = 10 * 1024 * 1024;
    if (typeof base64 === 'string' && base64.length > MAX_IMAGE_SIZE) {
      toast.error('Image too large (max 10MB)');
      return;
    }
    setPendingImage(base64);
    toast.success('Image attached');
  }, []);

  const handleTextDrop = useCallback((content: string, filename: string) => {
    setTextContext(`[Context File: ${filename}]\n\`\`\`\n${content}\n\`\`\`\n\nAnalyze the contents of this file.`);
    toast.success(`File "${filename}" added as context`);
  }, []);

  const handlePasteImage = useCallback((base64: string) => handleImageDrop(base64), [handleImageDrop]);

  // ----- Global paste handler -----------------------------------------

  useEffect(() => {
    const handleGlobalPaste = (e: ClipboardEvent) => {
      const active = document.activeElement;
      if (active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement) {
        return;
      }

      const items = e.clipboardData?.items;
      if (!items) return;

      for (const item of items) {
        if (item.type.startsWith('image/')) {
          const blob = item.getAsFile();
          if (blob) {
            const reader = new FileReader();
            reader.onload = (event) => {
              if (event.target?.result && typeof event.target.result === 'string') {
                handleImageDrop(event.target.result);
              }
            };
            reader.readAsDataURL(blob);
            e.preventDefault();
            return;
          }
        }
        if (item.kind === 'file' && !item.type.startsWith('image/')) {
          const file = item.getAsFile();
          if (file) {
            if (file.size > 5 * 1024 * 1024) {
              toast.error(`File "${file.name}" too large (max 5MB)`);
              return;
            }
            const reader = new FileReader();
            reader.onload = (event) => {
              if (event.target?.result && typeof event.target.result === 'string') {
                handleTextDrop(event.target.result.substring(0, 20_000), file.name);
              }
            };
            reader.readAsText(file);
            e.preventDefault();
            return;
          }
        }
      }
    };

    window.addEventListener('paste', handleGlobalPaste);
    return () => window.removeEventListener('paste', handleGlobalPaste);
  }, [handleImageDrop, handleTextDrop]);

  // ----- Submit handler -----------------------------------------------

  const handleSubmit = useCallback(
    (prompt: string, image: string | null) => {
      const finalPrompt = textContext ? `${textContext}\n\n${prompt}` : prompt;
      onSubmit(finalPrompt, image);
      setTextContext('');
      setPendingImage(null);
    },
    [onSubmit, textContext],
  );

  // ----- Context menu handlers ----------------------------------------

  const handleContextMenu = useCallback((e: ReactMouseEvent, message: Message) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, message });
  }, []);

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  const handleCopyMessage = useCallback(() => {
    if (contextMenu) {
      navigator.clipboard.writeText(contextMenu.message.content);
      toast.success('Copied to clipboard');
      handleCloseContextMenu();
    }
  }, [contextMenu, handleCloseContextMenu]);

  // ----- Render -------------------------------------------------------

  return (
    <DragDropZone onImageDrop={handleImageDrop} onTextDrop={handleTextDrop}>
      <div className="flex-1 w-full h-full flex flex-col min-h-0 relative gap-2">
        {/* Messages panel */}
        <div className={cn('flex-1 min-h-0 flex flex-col overflow-hidden rounded-xl', theme.glassPanel)}>
          <div ref={scrollContainerRef} className={cn('flex-1 min-h-0 overflow-y-auto', theme.scrollbar)}>
            {messages.length === 0 ? (
              <EmptyState />
            ) : (
              <>
                {messages.map((message, index) => (
                  <MessageBubble
                    key={`${message.timestamp}-${message.role}-${index}`}
                    message={message}
                    isLast={index === messages.length - 1}
                    isStreaming={isStreaming}
                    onContextMenu={handleContextMenu}
                  />
                ))}

                {/* Streaming typing indicator (shown when waiting for first token) */}
                <AnimatePresence>
                  {isStreaming && messages.length > 0 && messages[messages.length - 1]?.role === 'user' && (
                    <StreamingIndicator />
                  )}
                </AnimatePresence>

                {/* Scroll anchor */}
                <div ref={messagesEndRef} />
              </>
            )}
          </div>
        </div>

        {/* Context menu */}
        {contextMenu && (
          <ChatContextMenu
            x={contextMenu.x}
            y={contextMenu.y}
            isUser={contextMenu.message.role === 'user'}
            onClose={handleCloseContextMenu}
            onCopy={handleCopyMessage}
          />
        )}

        {/* Text context indicator */}
        <AnimatePresence>
          {textContext && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: 'auto' }}
              exit={{ opacity: 0, height: 0 }}
              className={cn(
                'shrink-0 px-4 py-2 rounded-xl flex items-center gap-2 text-xs',
                theme.glassPanel,
                theme.textMuted,
              )}
            >
              <FileText size={14} className={theme.accentText} />
              <span className="flex-1 truncate">Context file loaded ({Math.round(textContext.length / 1024)}KB)</span>
              <button
                type="button"
                onClick={() => setTextContext('')}
                className="p-1 rounded hover:bg-red-500/20 hover:text-red-400 transition-colors"
                title="Remove context"
              >
                <X size={12} />
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Input panel */}
        <div className={cn('shrink-0 rounded-xl', theme.glassPanel)}>
          <ChatInput
            isStreaming={isStreaming}
            onSubmit={handleSubmit}
            onStop={onStop}
            pendingImage={pendingImage}
            onClearImage={() => setPendingImage(null)}
            onPasteImage={handlePasteImage}
            onPasteFile={handleTextDrop}
          />
        </div>
      </div>
    </DragDropZone>
  );
});

ChatContainer.displayName = 'ChatContainer';

export default ChatContainer;
