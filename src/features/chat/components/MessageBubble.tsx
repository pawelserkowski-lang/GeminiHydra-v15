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
import {
  type ImgHTMLAttributes,
  isValidElement,
  memo,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useCallback,
  useMemo,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import remarkGfm from 'remark-gfm';
import { CodeBlock } from '@/components/molecules';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { chatLanguages } from '@/shared/utils/highlightLanguages';
import { type Message, useCurrentSessionId } from '@/stores/viewStore';
import { MessageRating } from './MessageRating';

// ---------------------------------------------------------------------------
// Helper: split content into text and tool-output segments (#41)
// ---------------------------------------------------------------------------

interface ContentSegment {
  type: 'text' | 'tool';
  name?: string;
  content: string;
}

function splitToolOutput(content: string): ContentSegment[] {
  const toolPattern = /\n---\n\*\*🔧 Tool:\*\* `([^`]+)`\n```\n([\s\S]*?)\n```\n---\n/g;
  const segments: ContentSegment[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  // biome-ignore lint/suspicious/noAssignInExpressions: standard regex exec loop pattern
  while ((match = toolPattern.exec(content)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ type: 'text', content: content.slice(lastIndex, match.index) });
    }
    segments.push({ type: 'tool', name: match[1] ?? '', content: match[2] ?? '' });
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < content.length) {
    segments.push({ type: 'text', content: content.slice(lastIndex) });
  }
  return segments;
}

// ---------------------------------------------------------------------------
// Helper: strip parallel execution header (now handled by ToolProgress) (#41)
// ---------------------------------------------------------------------------

function stripParallelHeader(content: string): string {
  return content.replace(/⚡ Parallel execution: \d+ tools\n?/g, '');
}

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

// ---------------------------------------------------------------------------
// Lazy-loading image with blur placeholder (#4)
// ---------------------------------------------------------------------------

function LazyImage(props: ImgHTMLAttributes<HTMLImageElement>) {
  const [loaded, setLoaded] = useState(false);

  return (
    <span className="relative inline-block">
      {/* Skeleton placeholder shown while image loads */}
      {!loaded && <span className="absolute inset-0 bg-gray-500/20 rounded-lg animate-pulse backdrop-blur-sm" />}
      <img
        {...props}
        loading="lazy"
        onLoad={(e) => {
          setLoaded(true);
          if (typeof props.onLoad === 'function') props.onLoad(e);
        }}
        className={cn(
          props.className,
          'rounded-lg max-w-full transition-opacity duration-300',
          loaded ? 'opacity-100' : 'opacity-0',
        )}
        alt={props.alt ?? 'Image'}
      />
    </span>
  );
}

// ============================================================================
// TYPES
// ============================================================================

interface MessageBubbleProps {
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
  const { t, i18n } = useTranslation();
  const theme = useViewTheme();
  const [copied, setCopied] = useState(false);
  const currentSessionId = useCurrentSessionId();

  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  // Locale-aware formatting: prefer i18next language, fall back to navigator.language
  const locale = useMemo(() => i18n.language || navigator.language, [i18n.language]);

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
    'relative max-w-[85%] rounded-2xl px-5 py-4',
    'text-[15px] leading-relaxed font-mono',
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
          title={t('chat.copyMessage', 'Copy message')}
          aria-label={t('chat.copyMessage', 'Copy message')}
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
            <span className="font-bold text-sm uppercase tracking-wider">
              {t('chat.systemOutput', 'System Output')}
            </span>
          </div>
        )}

        {/* Model badge */}
        {!isUser && !isSystem && message.model && (
          <div className="flex items-center gap-1.5 mb-1.5 pb-1 border-b border-matrix-accent/10">
            <Cpu size={11} className={cn(theme.accentText, 'opacity-70')} />
            <span className={cn('text-xs font-mono tracking-wide opacity-70', theme.accentText)}>{message.model}</span>
          </div>
        )}

        {/* Markdown content with tool output separation (#41) */}
        <div className="markdown-body prose prose-sm max-w-none break-words">
          {(() => {
            const cleaned = stripParallelHeader(message.content);
            const segments = splitToolOutput(cleaned);
            // If no tool segments found, render as single markdown block
            if (segments.length <= 1 && segments[0]?.type !== 'tool') {
              return (
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
                      const isInline =
                        !node?.position || (node.position.start.line === node.position.end.line && !match);
                      const codeContent = extractText(children).replace(/\n$/, '');
                      if (isInline) {
                        return (
                          <code className={cn(className, 'bg-black/20 px-1.5 py-0.5 rounded text-sm')}>{children}</code>
                        );
                      }
                      return <CodeBlock {...(match?.[1] != null && { language: match[1] })} code={codeContent} />;
                    },
                    pre({ children }: { children?: ReactNode | undefined }) {
                      return <>{children}</>;
                    },
                    img(imgProps: ImgHTMLAttributes<HTMLImageElement>) {
                      return <LazyImage {...imgProps} />;
                    },
                  }}
                >
                  {cleaned}
                </ReactMarkdown>
              );
            }
            // Render mixed text + tool segments
            return segments.map((segment) => {
              const segKey = `${segment.type}-${segment.type === 'tool' ? segment.name : segment.content.slice(0, 32)}`;
              if (segment.type === 'tool') {
                return (
                  <details key={segKey} className="my-2 rounded-lg border border-white/10 bg-black/20">
                    <summary className="cursor-pointer px-3 py-2 text-xs text-white/60 hover:text-white/80 flex items-center gap-2">
                      <Terminal className="w-3.5 h-3.5" />
                      <span>{t('chat.toolLabel', { name: segment.name })}</span>
                      <span className="ml-auto text-[10px]">
                        {t('chat.linesCount', { count: segment.content.split('\n').length })}
                      </span>
                    </summary>
                    <pre className="overflow-x-auto px-3 py-2 text-xs text-white/70 border-t border-white/5 max-h-60 overflow-y-auto">
                      <code>{segment.content}</code>
                    </pre>
                  </details>
                );
              }
              return (
                <ReactMarkdown
                  key={segKey}
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
                      const isInline =
                        !node?.position || (node.position.start.line === node.position.end.line && !match);
                      const codeContent = extractText(children).replace(/\n$/, '');
                      if (isInline) {
                        return (
                          <code className={cn(className, 'bg-black/20 px-1.5 py-0.5 rounded text-sm')}>{children}</code>
                        );
                      }
                      return <CodeBlock {...(match?.[1] != null && { language: match[1] })} code={codeContent} />;
                    },
                    pre({ children }: { children?: ReactNode | undefined }) {
                      return <>{children}</>;
                    },
                    img(imgProps: ImgHTMLAttributes<HTMLImageElement>) {
                      return <LazyImage {...imgProps} />;
                    },
                  }}
                >
                  {segment.content}
                </ReactMarkdown>
              );
            });
          })()}
        </div>

        {/* Response metadata badge (#42) */}
        {message.role === 'assistant' && !isStreaming && (
          <div className="mt-2 flex items-center gap-3 text-[10px] text-white/30">
            {message.content && (
              <span>
                {(message.content.length / 1000).toFixed(1)}K {t('chat.charsCount')}
              </span>
            )}
            {message.model && <span>{message.model}</span>}
          </div>
        )}

        {/* Timestamp */}
        {message.timestamp > 0 && (
          <div className={cn('text-xs mt-1.5 font-mono', theme.textMuted)}>
            {new Date(message.timestamp).toLocaleTimeString(locale, {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </div>
        )}

        {/* Star rating (assistant messages only, not while streaming) */}
        {message.role === 'assistant' && !isSystem && currentSessionId && !(isStreaming && isLast) && (
          <MessageRating messageId={String(message.timestamp)} sessionId={currentSessionId} />
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
