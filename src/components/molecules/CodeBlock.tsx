// src/components/molecules/CodeBlock.tsx
/**
 * CodeBlock Molecule
 * ==================
 * Syntax-highlighted code display with copy-to-clipboard, language badge,
 * optional line numbers, and glass-panel wrapper.
 *
 * Uses `hljs` CSS classes for syntax highlighting — works with rehype-highlight
 * when rendered inside react-markdown, and displays cleanly as plain code standalone.
 *
 * GeminiHydra-v15: White/neutral accent with --matrix-* CSS variables.
 */

import { Check, Clipboard, Terminal } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/shared/utils/cn';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CodeBlockProps {
  /** The code string to display. */
  code: string;
  /** Language identifier (e.g. 'typescript', 'python'). */
  language?: string;
  /** Show line numbers. Defaults to `false`. */
  showLineNumbers?: boolean;
  /** Maximum height before scrolling. Defaults to '24rem'. */
  maxHeight?: string;
  /** Extra CSS class on the root wrapper. */
  className?: string;
}

// ---------------------------------------------------------------------------
// Language display names
// ---------------------------------------------------------------------------

const LANGUAGE_NAMES: Record<string, string> = {
  js: 'JavaScript',
  javascript: 'JavaScript',
  ts: 'TypeScript',
  typescript: 'TypeScript',
  tsx: 'TSX',
  jsx: 'JSX',
  py: 'Python',
  python: 'Python',
  rs: 'Rust',
  rust: 'Rust',
  go: 'Go',
  java: 'Java',
  cpp: 'C++',
  c: 'C',
  cs: 'C#',
  csharp: 'C#',
  rb: 'Ruby',
  ruby: 'Ruby',
  php: 'PHP',
  swift: 'Swift',
  kt: 'Kotlin',
  kotlin: 'Kotlin',
  html: 'HTML',
  css: 'CSS',
  scss: 'SCSS',
  json: 'JSON',
  yaml: 'YAML',
  yml: 'YAML',
  xml: 'XML',
  md: 'Markdown',
  markdown: 'Markdown',
  sql: 'SQL',
  sh: 'Shell',
  shell: 'Shell',
  bash: 'Bash',
  powershell: 'PowerShell',
  dockerfile: 'Dockerfile',
  toml: 'TOML',
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/** Number of lines above which code blocks auto-collapse (#44) */
const AUTO_COLLAPSE_THRESHOLD = 20;

export const CodeBlock = memo(function CodeBlock({
  code,
  language,
  showLineNumbers = false,
  maxHeight = '24rem',
  className,
}: CodeBlockProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const preRef = useRef<HTMLPreElement>(null);

  const lang = language?.toLowerCase() ?? '';
  const displayName = LANGUAGE_NAMES[lang] ?? (lang ? lang.toUpperCase() : 'Code');

  // Split into lines for line-number rendering
  const lines = useMemo(() => code.split('\n'), [code]);

  // Auto-collapse large code blocks (#44)
  const isLong = lines.length > AUTO_COLLAPSE_THRESHOLD;
  const [expanded, setExpanded] = useState(!isLong);

  // ----- Copy to clipboard ---------------------------------------------

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback for older browsers
      const textarea = document.createElement('textarea');
      textarea.value = code;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [code]);

  // ----- Render --------------------------------------------------------

  return (
    <div className={cn('glass-panel overflow-hidden my-3 group', className)}>
      {/* Header bar */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-white/10 bg-black/20">
        {/* Language icon + label */}
        <div className="flex items-center gap-2">
          <Terminal size={14} className="text-[var(--matrix-text-dim)]" />
          <span className="text-xs font-mono text-[var(--matrix-text-dim)] uppercase tracking-wider">
            {displayName}
          </span>
        </div>

        {/* Copy button */}
        <button
          type="button"
          onClick={handleCopy}
          className={cn(
            'flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-mono transition-colors',
            'text-[var(--matrix-text-dim)] hover:text-[var(--matrix-accent)] hover:bg-white/10',
          )}
          aria-label={copied ? t('common.copied') : t('common.copyCode')}
        >
          <AnimatePresence mode="wait" initial={false}>
            {copied ? (
              <motion.span
                key="check"
                initial={{ scale: 0.5, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.5, opacity: 0 }}
                transition={{ duration: 0.15 }}
                className="flex items-center gap-1 text-[var(--matrix-success)]"
              >
                <Check size={14} />
                {t('common.copied')}
              </motion.span>
            ) : (
              <motion.span
                key="copy"
                initial={{ scale: 0.5, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.5, opacity: 0 }}
                transition={{ duration: 0.15 }}
                className="flex items-center gap-1"
              >
                <Clipboard size={14} />
                {t('common.copy')}
              </motion.span>
            )}
          </AnimatePresence>
        </button>
      </div>

      {/* Code content — auto-collapse for large blocks (#44) */}
      {isLong && !expanded ? (
        <>
          <div className="overflow-hidden relative" style={{ maxHeight: '20rem' }}>
            <pre
              ref={preRef}
              className={cn(
                'm-0 p-4 bg-transparent text-sm leading-relaxed',
                'font-mono text-[var(--matrix-text)]',
                showLineNumbers && 'flex',
              )}
            >
              {showLineNumbers && (
                <div
                  className="select-none pr-4 mr-4 border-r border-white/10 text-right text-[var(--matrix-text-dim)]"
                  aria-hidden="true"
                >
                  {lines.slice(0, AUTO_COLLAPSE_THRESHOLD).map((_line, i) => (
                    // biome-ignore lint/suspicious/noArrayIndexKey: Line numbers are static, never reordered
                    <div key={i} className="leading-relaxed">
                      {i + 1}
                    </div>
                  ))}
                </div>
              )}
              <code className={cn(lang && `language-${lang}`, 'block flex-1')}>
                {lines.slice(0, AUTO_COLLAPSE_THRESHOLD).join('\n')}
              </code>
            </pre>
            {/* Fade-out gradient overlay */}
            <div className="absolute bottom-0 left-0 right-0 h-12 bg-gradient-to-t from-black/60 to-transparent pointer-events-none" />
          </div>
          <button
            type="button"
            onClick={() => setExpanded(true)}
            className="w-full py-1.5 text-xs text-center text-white/50 hover:text-white/80 bg-black/30 border-t border-white/5 transition-colors"
          >
            {t('chat.showAllLines', { count: lines.length })}
          </button>
        </>
      ) : (
        <>
          <div className="overflow-auto" style={{ maxHeight: isLong ? '40rem' : maxHeight }}>
            <pre
              ref={preRef}
              className={cn(
                'm-0 p-4 bg-transparent text-sm leading-relaxed',
                'font-mono text-[var(--matrix-text)]',
                showLineNumbers && 'flex',
              )}
            >
              {/* Line numbers gutter */}
              {showLineNumbers && (
                <div
                  className="select-none pr-4 mr-4 border-r border-white/10 text-right text-[var(--matrix-text-dim)]"
                  aria-hidden="true"
                >
                  {lines.map((_line, i) => (
                    // biome-ignore lint/suspicious/noArrayIndexKey: Line numbers are static, never reordered
                    <div key={i} className="leading-relaxed">
                      {i + 1}
                    </div>
                  ))}
                </div>
              )}

              {/* Code body */}
              <code className={cn(lang && `language-${lang}`, 'block flex-1')}>{code}</code>
            </pre>
          </div>
          {isLong && expanded && (
            <button
              type="button"
              onClick={() => setExpanded(false)}
              className="w-full py-1.5 text-xs text-center text-white/50 hover:text-white/80 bg-black/30 border-t border-white/5 transition-colors"
            >
              {t('chat.collapse')}
            </button>
          )}
        </>
      )}
    </div>
  );
});
