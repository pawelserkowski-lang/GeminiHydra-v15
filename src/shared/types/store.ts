/** Shared Jaskier store types — keep in sync across all 3 projects. */

export interface ChatSession {
  id: string;
  title: string;
  createdAt: number;
  updatedAt?: number;
  messageCount?: number;
  preview?: string;
  /** Per-session working directory (empty = inherit from global settings) */
  workingDirectory?: string;
  /** Locked agent ID for this session (GH only) */
  agentId?: string;
}

export interface ChatTab {
  id: string;
  sessionId: string;
  title: string;
  isPinned: boolean;
}

export type MessageRole = 'user' | 'assistant' | 'system';

export interface ChatMessage {
  role: MessageRole;
  content: string;
  timestamp: number;
  model?: string;
}
