// src/stores/types.ts
export type View = 'home' | 'chat' | 'agents' | 'history' | 'settings' | 'status' | 'brain';

export interface Session {
  id: string;
  title: string;
  createdAt: number;
}

export interface ChatTab {
  id: string;
  sessionId: string;
  title: string;
  isPinned: boolean;
}

export type MessageRole = 'user' | 'assistant' | 'system';

export interface Message {
  role: MessageRole;
  content: string;
  timestamp: number;
  model?: string;
}
