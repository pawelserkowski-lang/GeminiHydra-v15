// src/workers/markdownWorker.ts
/**
 * Markdown Parsing Web Worker
 * ============================
 * Parses markdown to HTML off the main thread to prevent UI jank
 * for large chat messages. Uses a lightweight regex-based approach.
 */

// ---------------------------------------------------------------------------
// Message Types
// ---------------------------------------------------------------------------

export interface MarkdownWorkerRequest {
  id: string;
  content: string;
}

export interface MarkdownWorkerResponse {
  id: string;
  html: string;
}

// ---------------------------------------------------------------------------
// Lightweight Markdown-to-HTML Parser
// ---------------------------------------------------------------------------

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function parseMarkdownToHtml(content: string): string {
  let html = content;

  // Fenced code blocks (``` ... ```)
  html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_match, lang: string, code: string) => {
    const langAttr = lang ? ` data-language="${escapeHtml(lang)}"` : '';
    return `<pre${langAttr}><code>${escapeHtml(code.trimEnd())}</code></pre>`;
  });

  // Inline code
  html = html.replace(/`([^`\n]+)`/g, (_match, code: string) => {
    return `<code>${escapeHtml(code)}</code>`;
  });

  // Headings (h1-h6)
  html = html.replace(/^######\s+(.+)$/gm, '<h6>$1</h6>');
  html = html.replace(/^#####\s+(.+)$/gm, '<h5>$1</h5>');
  html = html.replace(/^####\s+(.+)$/gm, '<h4>$1</h4>');
  html = html.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>');

  // Bold + Italic
  html = html.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');

  // Bold
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/__(.+?)__/g, '<strong>$1</strong>');

  // Italic
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
  html = html.replace(/_(.+?)_/g, '<em>$1</em>');

  // Strikethrough
  html = html.replace(/~~(.+?)~~/g, '<del>$1</del>');

  // Blockquotes
  html = html.replace(/^>\s+(.+)$/gm, '<blockquote>$1</blockquote>');

  // Horizontal rules
  html = html.replace(/^---$/gm, '<hr />');
  html = html.replace(/^\*\*\*$/gm, '<hr />');

  // Unordered list items
  html = html.replace(/^[-*+]\s+(.+)$/gm, '<li>$1</li>');

  // Ordered list items
  html = html.replace(/^\d+\.\s+(.+)$/gm, '<li>$1</li>');

  // Wrap consecutive <li> in <ul>
  html = html.replace(/((?:<li>.*<\/li>\n?)+)/g, '<ul>$1</ul>');

  // Links
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" rel="noopener noreferrer">$1</a>');

  // Images
  html = html.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img src="$2" alt="$1" />');

  // Paragraphs: wrap lines not already in block elements
  html = html.replace(/^(?!<(?:h[1-6]|pre|blockquote|ul|ol|li|hr|div|p))(.+)$/gm, '<p>$1</p>');

  // Clean up extra newlines
  html = html.replace(/\n{2,}/g, '\n');

  return html.trim();
}

// ---------------------------------------------------------------------------
// Worker Message Handler
// ---------------------------------------------------------------------------

self.onmessage = (event: MessageEvent<MarkdownWorkerRequest>) => {
  const { id, content } = event.data;
  const html = parseMarkdownToHtml(content);
  const response: MarkdownWorkerResponse = { id, html };
  self.postMessage(response);
};
