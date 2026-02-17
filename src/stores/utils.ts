// src/stores/utils.ts
export const MAX_SESSIONS = 50;
export const MAX_MESSAGES_PER_SESSION = 500;
export const MAX_TITLE_LENGTH = 100;

export function sanitizeTitle(title: string, maxLen: number): string {
  return title.trim().slice(0, maxLen) || 'New Chat';
}

export function sanitizeContent(content: string, maxLen: number): string {
  return content.slice(0, maxLen);
}
