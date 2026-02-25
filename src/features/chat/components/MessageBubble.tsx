// src/features/chat/components/MessageBubble.tsx
/**
 * GeminiHydra v15 - MessageBubble
 * ================================
 * Individual chat message display with avatar, markdown rendering,
 * model badge, timestamp, copy button, and streaming cursor.
 *
 * Ported from legacy MessageItem (inside MessageList.tsx) with:
 * - react-markdown + remark-gfm + rehype-highlight
 * - CodeBlock molecule for fenced code blocks
 * - Glass-panel styling via useViewTheme
 */

import { Bot, Check, Copy, Cpu, Terminal, User } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { isValidElement, memo, type MouseEvent as ReactMouseEvent, type ReactNode, useCallback, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import remarkGfm from 'remark-gfm';

import { CodeBlock } from '@/components/molecules';
import { chatLanguages } from '@/shared/utils/highlightLanguages';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import type { Message } from '@/stores/viewStore';

// ---------------------------------------------------------------------------
// Helper: extract plain text from React children (handles rehype-highlight spans)
// ---------------------------------------------------------------------------

function extractText(node: ReactNode): string {
  if (typeof node === 'string') return node;
  if (typeof node === 'number') return String(node);
  if (!node || typeof node === 'boolean') return '';
  if (Array.isArray(node)) return node.map(extractText).join('');
  if (isValidElement(node)) {
    return extractText((node.props as { children?: ReactNode }).children);
  }
  return '';
}

// ============================================================================
// TYPES
// ============================================================================

export interface MessageBubbleProps {
  message: Message;
  /** Whether this is the last message in the list. */
  isLast: boolean;
  /** Whether the assistant is currently streaming. */
  isStreaming: boolean;
  /** Context menu handler. */
  onContextMenu?: (e: ReactMouseEvent, message: Message) => void;
}

// ============================================================================
// ANIMATION
// ============================================================================

const bubbleVariants = {
  hidden: { opacity: 0, y: 6 },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.25, ease: 'easeOut' as const },
  },
};

// ============================================================================
// COMPONENT
// ============================================================================

export const MessageBubble = memo<MessageBubbleProps>(({ message, isLast, isStreaming, onContextMenu }) => {
  const theme = useViewTheme();
  const [copied, setCopied] = useState(false);

  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  // ----- Copy to clipboard -------------------------------------------

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(message.content);
    } catch {
      const textarea = document.createElement('textarea');
      textarea.value = message.content;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [message.content]);

  // ----- Context menu ------------------------------------------------

  const handleContextMenu = useCallback(
    (e: ReactMouseEvent) => {
      onContextMenu?.(e, message);
    },
    [onContextMenu, message],
  );

  // ----- Bubble class ------------------------------------------------

  const bubbleClass = cn(
    'relative max-w-[85%] rounded-2xl px-4 py-3',
    'text-base leading-relaxed font-mono',
    isUser && [
      theme.isLight
        ? 'bg-emerald-500/15 border border-emerald-500/20 text-black'
        : 'bg-white/10 border border-white/20 text-white',
    ],
    !isUser &&
      !isSystem && [
        theme.isLight
          ? 'bg-white/50 border border-white/30 text-black shadow-sm'
          : 'bg-black/40 border border-white/10 text-white shadow-lg',
      ],
    isSystem && [
      theme.isLight
        ? 'bg-amber-500/10 border border-amber-500/20 text-black'
        : 'bg-amber-500/10 border border-amber-500/20 text-white',
    ],
  );

  // ----- Render --------------------------------------------------------

  return (
    <motion.div
      variants={bubbleVariants}
      initial="hidden"
      animate="visible"
      className={cn('flex items-end gap-2 py-2 px-4 group relative', isUser ? 'justify-end' : 'justify-start')}
      onContextMenu={handleContextMenu}
    >
      {/* Assistant avatar */}
      {!isUser && !isSystem && (
        <div className={cn('flex-shrink-0 w-7 h-7 rounded-lg flex items-center justify-center mb-1', theme.accentBg)}>
          <Bot size={14} className={theme.accentText} />
        </div>
      )}

      {/* Message bubble */}
      <div className={bubbleClass}>
        {/* Copy button (top-right, revealed on hover) */}
        <button
          type="button"
          onClick={handleCopy}
          className={cn(
            'absolute top-2 right-2 p-1.5 rounded-lg z-20',
            'bg-black/30 text-white/80 backdrop-blur-sm shadow-sm',
            'hover:bg-[var(--matrix-accent)] hover:text-black',
            'opacity-0 group-hover:opacity-100 transition-all duration-200',
            'transform hover:scale-110',
          )}
          title="Copy message"
        >
          <AnimatePresence mode="wait" initial={false}>
            {copied ? (
              <motion.span
                key="check"
                initial={{ scale: 0.5, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.5, opacity: 0 }}
                transition={{ duration: 0.12 }}
              >
                <Check size={14} className="text-green-400" />
              </motion.span>
            ) : (
              <motion.span
                key="copy"
                initial={{ scale: 0.5, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.5, opacity: 0 }}
                transition={{ duration: 0.12 }}
              >
                <Copy size={14} />
              </motion.span>
            )}
          </AnimatePresence>
        </button>

        {/* System header */}
        {isSystem && (
          <div className="flex items-center gap-2 mb-1.5 border-b border-matrix-accent/15 pb-1.5 text-matrix-accent/70">
            <Terminal size={14} />
            <span className="font-bold text-sm uppercase tracking-wider">System Output</span>
          </div>
        )}

        {/* Model badge */}
        {!isUser && !isSystem && message.model && (
          <div className="flex items-center gap-1.5 mb-1.5 pb-1 border-b border-matrix-accent/10">
            <Cpu size={11} className={cn(theme.accentText, 'opacity-70')} />
            <span className={cn('text-xs font-mono tracking-wide opacity-70', theme.accentText)}>{message.model}</span>
          </div>
        )}

        {/* Markdown content */}
        <div className="markdown-body prose prose-sm max-w-none break-words">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            rehypePlugins={[[rehypeHighlight, { languages: chatLanguages }]]}
            components={{
              code({
                className,
                children,
                node,
              }: {
                className?: string | undefined;
                children?: ReactNode | undefined;
                node?: { position?: { start: { line: number }; end: { line: number } } } | undefined;
              }) {
                const match = /language-(\w+)/.exec(className ?? '');
                const isInline = !node?.position || (node.position.start.line === node.position.end.line && !match);
                const codeContent = extractText(children).replace(/\n$/, '');

                if (isInline) {
                  return (
                    <code className={cn(className, 'bg-black/20 px-1.5 py-0.5 rounded text-sm')}>
                      {children}
                    </code>
                  );
                }

                return <CodeBlock {...(match?.[1] != null && { language: match[1] })} code={codeContent} />;
              },
              pre({ children }: { children?: ReactNode | undefined }) {
                return <>{children}</>;
              },
            }}
          >
            {message.content}
          </ReactMarkdown>
        </div>

        {/* Timestamp */}
        {message.timestamp > 0 && (
          <div className={cn('text-xs mt-1.5 font-mono', theme.textMuted)}>
            {new Date(message.timestamp).toLocaleTimeString('en-US', {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </div>
        )}

        {/* Streaming cursor */}
        {message.role === 'assistant' && isStreaming && isLast && (
          <span className="inline-block w-1.5 h-3.5 ml-1 rounded-sm bg-[var(--matrix-accent)] animate-pulse align-middle" />
        )}
      </div>

      {/* User avatar */}
      {isUser && (
        <div
          className={cn(
            'flex-shrink-0 w-7 h-7 rounded-lg flex items-center justify-center mb-1',
            theme.isLight ? 'bg-emerald-500/15' : 'bg-matrix-accent/15',
          )}
        >
          <User size={14} className={theme.accentText} />
        </div>
      )}
    </motion.div>
  );
});

MessageBubble.displayName = 'MessageBubble';

export default MessageBubble;
