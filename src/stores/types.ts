import type {
  ChatMessage,
  ChatSession,
  ChatTab as SharedChatTab,
  MessageRole as SharedMessageRole,
} from '@/shared/types/store';

export type View = 'home' | 'login' | 'chat' | 'agents' | 'brain' | 'settings' | 'logs';

export type ViewId = View;

export interface Artifact {
  id: string;
  code: string;
  language: string;
  title?: string;
}

/**
 * GeminiHydra session
 */
export type Session = Pick<ChatSession, 'id' | 'title' | 'createdAt' | 'workingDirectory' | 'agentId'>;

export type { SharedChatTab as ChatTab };

export type MessageRole = SharedMessageRole;

export type Message = ChatMessage;
