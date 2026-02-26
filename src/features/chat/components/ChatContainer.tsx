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

import { Check, ClipboardList, Copy, FileText, MessageSquare, Paperclip, Trash2, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import {
  type DragEvent,
  memo,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { EmptyState } from '@/components/molecules/EmptyState';
import { useSettingsQuery } from '@/features/settings/hooks/useSettings';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type Message, useViewStore } from '@/stores/viewStore';

import { useFileReadMutation } from '../hooks/useFiles';
import { type AgentActivity, AgentActivityPanel } from './AgentActivityPanel';
import { ChatInput } from './ChatInput';
import { MessageBubble } from './MessageBubble';

// ============================================================================
// TYPES
// ============================================================================

interface ChatContainerProps {
  /** Whether the assistant is currently streaming. */
  isStreaming: boolean;
  /** Callback to submit a new message. */
  onSubmit: (prompt: string, image: string | null) => void;
  /** Callback to stop active stream. */
  onStop?: () => void;
  /** Live agent activity data. */
  agentActivity?: AgentActivity;
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
  const { t } = useTranslation();
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
      aria-label={t('chat.fileDropZone', 'File drop zone')}
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
              <span>{t('chat.dropFileToAddContext', 'DROP FILE TO ADD CONTEXT')}</span>
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
// EMPTY STATE (uses shared EmptyState molecule)
// ============================================================================

const ChatEmptyState = memo(() => {
  const { t } = useTranslation();
  return (
    <EmptyState
      icon={MessageSquare}
      title={t('chat.emptyState', 'Start a conversation')}
      description={t('chat.emptyStateDesc', 'Type a message or drop a file to begin.')}
      className="h-full"
    />
  );
});

ChatEmptyState.displayName = 'ChatEmptyState';

// ============================================================================
// STREAMING INDICATOR
// ============================================================================

const StreamingIndicator = memo(() => {
  const { t } = useTranslation();
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
      <span className={cn('text-xs font-mono', theme.textMuted)}>{t('chat.generating', 'Generating...')}</span>
    </motion.div>
  );
});

StreamingIndicator.displayName = 'StreamingIndicator';

// ============================================================================
// CHAT CONTAINER
// ============================================================================

export const ChatContainer = memo<ChatContainerProps>(({ isStreaming, onSubmit, onStop, agentActivity }) => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const { data: settings } = useSettingsQuery();

  // Store
  const currentSessionId = useViewStore((s) => s.currentSessionId);
  const chatHistory = useViewStore((s) => s.chatHistory);
  const messages = useMemo<Message[]>(
    () => (currentSessionId ? (chatHistory[currentSessionId] ?? []) : []),
    [currentSessionId, chatHistory],
  );

  // File read mutation
  const fileReadMutation = useFileReadMutation();

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

  // ----- Attach file by path ------------------------------------------

  const handleAttachPath = useCallback(
    (path: string) => {
      fileReadMutation.mutate(
        { path },
        {
          onSuccess: (data) => {
            if ('error' in data) {
              toast.error(`Cannot read file: ${(data as { error: string }).error}`);
              return;
            }
            const filename = path.split(/[\\/]/).pop() ?? path;
            setTextContext(
              `[File: ${filename}]\n\`\`\`\n${data.content}\n\`\`\`\n\nAnalyze the contents of this file.`,
            );
            toast.success(`File "${filename}" loaded as context${data.truncated ? ' (truncated)' : ''}`);
          },
          onError: (err) => {
            toast.error(`Failed to read file: ${err.message}`);
          },
        },
      );
    },
    [fileReadMutation],
  );

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

  // ----- Prompt history for arrow-key navigation ----------------------

  const promptHistory = useMemo(() => messages.filter((m) => m.role === 'user').map((m) => m.content), [messages]);

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

  // ----- Copy entire session -----------------------------------------------

  const [sessionCopied, setSessionCopied] = useState(false);

  const handleCopySession = useCallback(async () => {
    if (messages.length === 0) return;

    const session = useViewStore.getState().sessions.find((s) => s.id === currentSessionId);
    const title = session?.title ?? 'Untitled';
    const date = session ? new Date(session.createdAt).toLocaleString() : '';

    const lines = [`=== ${title} ===`, date ? `Date: ${date}` : '', `Messages: ${messages.length}`, ''];
    for (const msg of messages) {
      const role = msg.role === 'user' ? 'User' : msg.role === 'assistant' ? 'Assistant' : 'System';
      const time = new Date(msg.timestamp).toLocaleTimeString();
      const model = msg.model ? ` (${msg.model})` : '';
      lines.push(`[${role}] ${time}${model}:`);
      lines.push(msg.content);
      lines.push('');
    }

    const text = lines.join('\n');
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
    }
    toast.success('Session copied to clipboard');
    setSessionCopied(true);
    setTimeout(() => setSessionCopied(false), 2000);
  }, [messages, currentSessionId]);

  // ----- Render -------------------------------------------------------

  return (
    <DragDropZone onImageDrop={handleImageDrop} onTextDrop={handleTextDrop}>
      <div className="flex-1 w-full h-full flex flex-col min-h-0 relative gap-2">
        {/* Messages panel */}
        <div className={cn('flex-1 min-h-0 flex flex-col overflow-hidden rounded-xl relative', theme.glassPanel)}>
          {/* Copy session button */}
          {messages.length > 0 && (
            <button
              type="button"
              onClick={handleCopySession}
              title={t('chat.copySession', 'Copy entire session')}
              className={cn(
                'absolute top-2 right-3 z-10 p-1.5 rounded-lg transition-all',
                'opacity-40 hover:opacity-100',
                'hover:bg-[var(--matrix-accent)]/10',
                theme.textMuted,
              )}
            >
              {sessionCopied ? <Check size={16} className="text-emerald-400" /> : <ClipboardList size={16} />}
            </button>
          )}
          <div ref={scrollContainerRef} className={cn('flex-1 min-h-0 overflow-y-auto', theme.scrollbar)}>
            {messages.length === 0 ? (
              settings?.welcome_message ? (
                <MessageBubble
                  message={{
                    role: 'assistant',
                    content: settings.welcome_message,
                    timestamp: Date.now(),
                  }}
                  isLast={true}
                  isStreaming={false}
                  onContextMenu={() => {}}
                />
              ) : (
                <ChatEmptyState />
              )
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

        {/* Agent activity panel (live tool calls, plan steps) */}
        <AnimatePresence>
          {agentActivity && <AgentActivityPanel activity={agentActivity} />}
        </AnimatePresence>

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
                title={t('chat.removeContext', 'Remove context')}
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
            {...(onStop !== undefined && { onStop })}
            pendingImage={pendingImage}
            onClearImage={() => setPendingImage(null)}
            onPasteImage={handlePasteImage}
            onPasteFile={handleTextDrop}
            onAttachPath={handleAttachPath}
            promptHistory={promptHistory}
          />
        </div>
      </div>
    </DragDropZone>
  );
});

ChatContainer.displayName = 'ChatContainer';

export default ChatContainer;
