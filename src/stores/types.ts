// src/stores/types.ts
import type {
  ChatSession,
  ChatTab as SharedChatTab,
  ChatMessage,
  MessageRole as SharedMessageRole,
} from '@/shared/types/store';

export type View = 'home' | 'chat';

/**
 * GeminiHydra session — extends shared ChatSession.
 * Local alias `Session` kept for backward compatibility across all slices.
 */
export type Session = Pick<ChatSession, 'id' | 'title' | 'createdAt'>;

export type { SharedChatTab as ChatTab };

export type MessageRole = SharedMessageRole;

/**
 * GeminiHydra message — matches shared ChatMessage.
 * Local alias `Message` kept for backward compatibility.
 */
export type Message = ChatMessage;
