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

import { useVirtualizer } from '@tanstack/react-virtual';
import { Check, ClipboardList, FileText, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import {
  lazy,
  memo,
  type MouseEvent as ReactMouseEvent,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useSettingsQuery } from '@/features/settings/hooks/useSettings';
import { useOnlineStatus } from '@/shared/hooks/useOnlineStatus';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type Message, useViewStore } from '@/stores/viewStore';

import { useFileReadMutation } from '../hooks/useFiles';
import type { OrchestrationState } from '../hooks/useOrchestration';
import { usePromptHistory } from '../hooks/usePromptHistory';
import type { AgentActivity } from './AgentActivityPanel';
import { ChatContextMenu } from './ChatContextMenu';
import { ChatEmptyState } from './ChatEmptyState';
import { ChatInput } from './ChatInput';
import { DragDropZone } from './DragDropZone';
import { MessageBubble } from './MessageBubble';
import { NewMessagesButton } from './NewMessagesButton';
import { OfflineBanner } from './OfflineBanner';
import { SearchOverlay } from './SearchOverlay';
import { StreamingIndicator } from './StreamingIndicator';

// Lazy-loaded panels — only downloaded when agent activity or orchestration is active
const AgentActivityPanel = lazy(() => import('./AgentActivityPanel').then((m) => ({ default: m.AgentActivityPanel })));
const OrchestrationPanel = lazy(() => import('./OrchestrationPanel').then((m) => ({ default: m.OrchestrationPanel })));

// ============================================================================
// TYPES
// ============================================================================

interface ChatContainerProps {
  /** Whether the assistant is currently streaming. */
  isStreaming: boolean;
  /** Callback to submit a new message. */
  onSubmit: (prompt: string, image: string | null) => void;
  /** Callback for orchestrated submissions. */
  onOrchestrate?: (prompt: string, pattern: string) => void;
  /** Callback to stop active stream. */
  onStop?: () => void;
  /** Live agent activity data. */
  agentActivity?: AgentActivity;
  /** ADK orchestration state. */
  orchestration?: OrchestrationState;
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
// CHAT CONTAINER
// ============================================================================

export const ChatContainer = memo<ChatContainerProps>(
  ({ isStreaming, onSubmit, onOrchestrate, onStop, agentActivity, orchestration }) => {
    const { t } = useTranslation();
    const theme = useViewTheme();
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const scrollContainerRef = useRef<HTMLDivElement>(null);
    const { data: settings } = useSettingsQuery();

    // Store
    const currentSessionId = useViewStore((s) => s.currentSessionId);
    const chatHistory = useViewStore((s) => s.chatHistory);
    const currentSession = useViewStore((s) => s.sessions.find((sess) => sess.id === s.currentSessionId));
    const setSessionWorkingDirectory = useViewStore((s) => s.setSessionWorkingDirectory);
    const messages = useMemo<Message[]>(
      () => (currentSessionId ? (chatHistory[currentSessionId] ?? []) : []),
      [currentSessionId, chatHistory],
    );

    // File read mutation
    const fileReadMutation = useFileReadMutation();

    // Online status (#25)
    const isOnline = useOnlineStatus();

    // Local state
    const [pendingImage, setPendingImage] = useState<string | null>(null);
    const [textContext, setTextContext] = useState('');
    const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
    const [suggestion, setSuggestion] = useState({ text: '', key: 0 });

    // Search overlay state (#19)
    const [searchOpen, setSearchOpen] = useState(false);

    // ----- Ctrl+F search shortcut (#19) ---------------------------------

    useEffect(() => {
      const handleKeyDown = (e: KeyboardEvent) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
          e.preventDefault();
          setSearchOpen(true);
        }
      };
      window.addEventListener('keydown', handleKeyDown);
      return () => window.removeEventListener('keydown', handleKeyDown);
    }, []);

    // ----- Virtualizer for message list -----------------------------------

    const virtualizer = useVirtualizer({
      count: messages.length,
      getScrollElement: () => scrollContainerRef.current,
      estimateSize: () => 120,
      overscan: 5,
      measureElement: (el) => el.getBoundingClientRect().height,
    });

    // ----- Auto-scroll to bottom ----------------------------------------

    const scrollToBottom = useCallback(() => {
      if (messages.length > 0) {
        virtualizer.scrollToIndex(messages.length - 1, { align: 'end', behavior: 'smooth' });
      }
    }, [messages.length, virtualizer]);

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
        // Block submission when offline (#25)
        if (!isOnline) {
          toast.warning(t('chat.offlineBlocked', 'You are offline — message not sent'));
          return;
        }
        const finalPrompt = textContext ? `${textContext}\n\n${prompt}` : prompt;
        onSubmit(finalPrompt, image);
        setTextContext('');
        setPendingImage(null);
      },
      [onSubmit, textContext, isOnline, t],
    );

    // ----- Per-session working directory --------------------------------

    const handleWorkingDirectoryChange = useCallback(
      (wd: string) => {
        if (currentSessionId) {
          setSessionWorkingDirectory(currentSessionId, wd);
        }
      },
      [currentSessionId, setSessionWorkingDirectory],
    );

    const handleSuggestionSelect = useCallback(
      (text: string) => setSuggestion((prev) => ({ text, key: prev.key + 1 })),
      [],
    );

    // ----- Prompt history for arrow-key navigation (global, SQL-backed) --

    const { promptHistory } = usePromptHistory();

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
          {/* Offline banner (#25) */}
          <OfflineBanner />

          {/* Messages panel */}
          <div className={cn('flex-1 min-h-0 flex flex-col overflow-hidden rounded-xl relative', theme.glassPanel)}>
            {/* Search overlay (#19) */}
            <SearchOverlay
              messages={messages}
              scrollContainerRef={scrollContainerRef}
              isOpen={searchOpen}
              onClose={() => setSearchOpen(false)}
            />

            {/* Copy session button */}
            {messages.length > 0 && (
              <button
                type="button"
                onClick={handleCopySession}
                title={t('chat.copySession', 'Copy entire session')}
                className={cn(
                  'absolute top-2 z-10 p-1.5 rounded-lg transition-all',
                  'opacity-40 hover:opacity-100',
                  'hover:bg-[var(--matrix-accent)]/10',
                  theme.textMuted,
                  searchOpen ? 'right-[280px]' : 'right-3',
                )}
              >
                {sessionCopied ? <Check size={16} className="text-emerald-400" /> : <ClipboardList size={16} />}
              </button>
            )}
            <div
              ref={scrollContainerRef}
              role="log"
              aria-live="polite"
              aria-label={t('chat.messageHistory', 'Chat message history')}
              className={cn('flex-1 min-h-0 overflow-y-auto', theme.scrollbar)}
            >
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
                  <ChatEmptyState onSuggestionSelect={handleSuggestionSelect} />
                )
              ) : (
                <>
                  {/* Virtualized message list */}
                  <div
                    style={{
                      height: `${virtualizer.getTotalSize()}px`,
                      width: '100%',
                      position: 'relative',
                    }}
                  >
                    {virtualizer.getVirtualItems().map((virtualItem) => {
                      const message = messages[virtualItem.index];
                      if (!message) return null;
                      return (
                        <div
                          key={virtualItem.key}
                          data-index={virtualItem.index}
                          ref={virtualizer.measureElement}
                          style={{
                            position: 'absolute',
                            top: 0,
                            left: 0,
                            width: '100%',
                            transform: `translateY(${virtualItem.start}px)`,
                          }}
                        >
                          <MessageBubble
                            message={message}
                            isLast={virtualItem.index === messages.length - 1}
                            isStreaming={isStreaming}
                            onContextMenu={handleContextMenu}
                          />
                        </div>
                      );
                    })}
                  </div>

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

            {/* New messages floating button (#20) */}
            <NewMessagesButton
              scrollAnchorRef={messagesEndRef}
              scrollContainerRef={scrollContainerRef}
              messageCount={messages.length}
            />
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

          {/* Orchestration panel (ADK multi-agent pipeline) — lazy-loaded */}
          <AnimatePresence>
            {orchestration && (
              <Suspense fallback={null}>
                <OrchestrationPanel orchestration={orchestration} />
              </Suspense>
            )}
          </AnimatePresence>

          {/* Agent activity panel (live tool calls, plan steps) — lazy-loaded */}
          <AnimatePresence>
            {agentActivity && (
              <Suspense fallback={null}>
                <AgentActivityPanel activity={agentActivity} />
              </Suspense>
            )}
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
              {...(onOrchestrate !== undefined && { onOrchestrate })}
              {...(onStop !== undefined && { onStop })}
              pendingImage={pendingImage}
              onClearImage={() => setPendingImage(null)}
              onPasteImage={handlePasteImage}
              onPasteFile={handleTextDrop}
              onAttachPath={handleAttachPath}
              promptHistory={promptHistory}
              sessionId={currentSessionId ?? undefined}
              workingDirectory={currentSession?.workingDirectory}
              onWorkingDirectoryChange={handleWorkingDirectoryChange}
              initialValue={suggestion.text}
              initialValueKey={suggestion.key}
            />
          </div>
        </div>
      </DragDropZone>
    );
  },
);

ChatContainer.displayName = 'ChatContainer';

export default ChatContainer;
