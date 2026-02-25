// src/shared/utils/highlightLanguages.ts
/**
 * Selective highlight.js language registration for rehype-highlight.
 * Imports only the languages commonly used in AI chat code blocks,
 * reducing bundle size from ~376KB (all) to ~60KB.
 */
import type { LanguageFn } from 'highlight.js';

import bash from 'highlight.js/lib/languages/bash';
import css from 'highlight.js/lib/languages/css';
import go from 'highlight.js/lib/languages/go';
import java from 'highlight.js/lib/languages/java';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import markdown from 'highlight.js/lib/languages/markdown';
import python from 'highlight.js/lib/languages/python';
import rust from 'highlight.js/lib/languages/rust';
import sql from 'highlight.js/lib/languages/sql';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';

export const chatLanguages: Record<string, LanguageFn> = {
  bash,
  css,
  go,
  java,
  javascript,
  json,
  markdown,
  python,
  rust,
  shell: bash,
  sql,
  typescript,
  xml,
  html: xml,
  yaml,
};
