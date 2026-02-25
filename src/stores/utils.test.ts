import { describe, it, expect } from 'vitest';
import { sanitizeTitle, sanitizeContent, MAX_SESSIONS, MAX_MESSAGES_PER_SESSION, MAX_TITLE_LENGTH } from './utils';

describe('store utils', () => {
  describe('constants', () => {
    it('MAX_SESSIONS is 50', () => {
      expect(MAX_SESSIONS).toBe(50);
    });

    it('MAX_MESSAGES_PER_SESSION is 500', () => {
      expect(MAX_MESSAGES_PER_SESSION).toBe(500);
    });

    it('MAX_TITLE_LENGTH is 100', () => {
      expect(MAX_TITLE_LENGTH).toBe(100);
    });
  });

  describe('sanitizeTitle', () => {
    it('trims whitespace', () => {
      expect(sanitizeTitle('  hello  ', 100)).toBe('hello');
    });

    it('truncates to maxLen', () => {
      const long = 'A'.repeat(200);
      expect(sanitizeTitle(long, 50).length).toBe(50);
    });

    it('returns "New Chat" for empty string', () => {
      expect(sanitizeTitle('', 100)).toBe('New Chat');
    });

    it('returns "New Chat" for whitespace-only string', () => {
      expect(sanitizeTitle('   ', 100)).toBe('New Chat');
    });

    it('keeps short titles unchanged', () => {
      expect(sanitizeTitle('Hello World', 100)).toBe('Hello World');
    });
  });

  describe('sanitizeContent', () => {
    it('returns content unchanged if under limit', () => {
      expect(sanitizeContent('hello', 100)).toBe('hello');
    });

    it('truncates content exceeding maxLen', () => {
      const long = 'B'.repeat(200);
      expect(sanitizeContent(long, 50).length).toBe(50);
    });

    it('handles empty string', () => {
      expect(sanitizeContent('', 100)).toBe('');
    });
  });
});
