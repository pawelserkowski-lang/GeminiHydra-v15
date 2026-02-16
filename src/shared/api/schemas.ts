// src/shared/api/schemas.ts
/**
 * GeminiHydra v15 - Zod v4 Schemas
 * ==================================
 * Typed schemas for every backend API endpoint.
 * All types are inferred from schemas — zero manual `interface` duplication.
 */

import { z } from 'zod';

// ============================================================================
// HEALTH
// ============================================================================

export const healthSchema = z.object({
  status: z.string(),
  version: z.string(),
  uptime_seconds: z.number(),
});

export type Health = z.infer<typeof healthSchema>;

export const detailedHealthSchema = healthSchema.extend({
  system: z.object({
    cpu_usage: z.number(),
    memory_used: z.number(),
    memory_total: z.number(),
    os: z.string(),
  }),
});

export type DetailedHealth = z.infer<typeof detailedHealthSchema>;

// ============================================================================
// AGENTS
// ============================================================================

export const agentSchema = z.object({
  id: z.string(),
  name: z.string(),
  role: z.string(),
  specialization: z.string(),
  tier: z.string(),
  status: z.string(),
  description: z.string(),
});

export type Agent = z.infer<typeof agentSchema>;

export const agentsListSchema = z.array(agentSchema);

export type AgentsList = z.infer<typeof agentsListSchema>;

// ============================================================================
// CLASSIFY
// ============================================================================

export const classifyResponseSchema = z.object({
  agent_id: z.string(),
  agent_name: z.string(),
  confidence: z.number(),
  reasoning: z.string(),
});

export type ClassifyResponse = z.infer<typeof classifyResponseSchema>;

// ============================================================================
// EXECUTE
// ============================================================================

export const executeResponseSchema = z.object({
  id: z.string(),
  result: z.string(),
  mode: z.string(),
  duration_ms: z.number(),
  plan: z
    .object({
      agent: z.string().optional(),
      steps: z.array(z.string()),
      estimated_time: z.string().optional(),
    })
    .optional(),
  files_loaded: z.array(z.string()).optional(),
});

export type ExecuteResponse = z.infer<typeof executeResponseSchema>;

// ============================================================================
// FILES
// ============================================================================

export const fileReadResponseSchema = z.object({
  path: z.string(),
  content: z.string(),
  size_bytes: z.number(),
  truncated: z.boolean(),
  extension: z.string(),
});

export type FileReadResponse = z.infer<typeof fileReadResponseSchema>;

export const fileEntrySchema = z.object({
  name: z.string(),
  path: z.string(),
  is_dir: z.boolean(),
  size_bytes: z.number(),
  extension: z.string().nullable().optional(),
});

export type FileEntry = z.infer<typeof fileEntrySchema>;

export const fileListResponseSchema = z.object({
  path: z.string(),
  entries: z.array(fileEntrySchema),
  count: z.number(),
});

export type FileListResponse = z.infer<typeof fileListResponseSchema>;

// ============================================================================
// GEMINI MODELS
// ============================================================================

export const geminiModelSchema = z.object({
  name: z.string(),
  display_name: z.string(),
  description: z.string(),
  supported_methods: z.array(z.string()),
});

export type GeminiModel = z.infer<typeof geminiModelSchema>;

export const geminiModelsSchema = z.object({
  models: z.array(geminiModelSchema),
});

export type GeminiModels = z.infer<typeof geminiModelsSchema>;

// ============================================================================
// SYSTEM STATS
// ============================================================================

export const systemStatsSchema = z.object({
  cpu_usage: z.number(),
  memory_used: z.number(),
  memory_total: z.number(),
  uptime_seconds: z.number(),
  active_agents: z.number(),
  total_requests: z.number(),
});

export type SystemStats = z.infer<typeof systemStatsSchema>;

// ============================================================================
// HISTORY
// ============================================================================

export const historyMessageSchema = z.object({
  role: z.string(),
  content: z.string(),
  timestamp: z.string(),
  model: z.string().optional(),
});

export type HistoryMessage = z.infer<typeof historyMessageSchema>;

export const historyEntrySchema = z.object({
  id: z.string(),
  session_id: z.string(),
  messages: z.array(historyMessageSchema),
  created_at: z.string(),
});

export type HistoryEntry = z.infer<typeof historyEntrySchema>;

export const historyListSchema = z.array(historyEntrySchema);

export type HistoryList = z.infer<typeof historyListSchema>;

// ============================================================================
// SETTINGS
// ============================================================================

export const settingsSchema = z
  .object({
    temperature: z.number(),
    max_tokens: z.number(),
    default_model: z.string(),
    language: z.string(),
    theme: z.string(),
    welcome_message: z.string().optional().default(''),
  })
  .passthrough();

export type Settings = z.infer<typeof settingsSchema>;

// ============================================================================
// MEMORY
// ============================================================================

export const memoryEntrySchema = z.object({
  id: z.string(),
  content: z.string(),
  created_at: z.string(),
  agent_id: z.string(),
});

export type MemoryEntry = z.infer<typeof memoryEntrySchema>;

// ============================================================================
// KNOWLEDGE GRAPH
// ============================================================================

export const knowledgeNodeSchema = z.object({
  id: z.string(),
  label: z.string(),
  node_type: z.string(),
});

export type KnowledgeNode = z.infer<typeof knowledgeNodeSchema>;

export const knowledgeEdgeSchema = z.object({
  source_id: z.string(),
  target_id: z.string(),
  relation: z.string(),
});

export type KnowledgeEdge = z.infer<typeof knowledgeEdgeSchema>;

// ============================================================================
// WEBSOCKET PROTOCOL
// ============================================================================

export const wsStartMessageSchema = z.object({
  type: z.literal('start'),
  id: z.string(),
  agent: z.string(),
  model: z.string(),
  files_loaded: z.array(z.string()),
});

export type WsStartMessage = z.infer<typeof wsStartMessageSchema>;

export const wsTokenMessageSchema = z.object({
  type: z.literal('token'),
  content: z.string(),
});

export type WsTokenMessage = z.infer<typeof wsTokenMessageSchema>;

export const wsPlanMessageSchema = z.object({
  type: z.literal('plan'),
  agent: z.string(),
  confidence: z.number(),
  steps: z.array(z.string()),
});

export type WsPlanMessage = z.infer<typeof wsPlanMessageSchema>;

export const wsCompleteMessageSchema = z.object({
  type: z.literal('complete'),
  duration_ms: z.number(),
});

export type WsCompleteMessage = z.infer<typeof wsCompleteMessageSchema>;

export const wsErrorMessageSchema = z.object({
  type: z.literal('error'),
  message: z.string(),
  code: z.string().optional(),
});

export type WsErrorMessage = z.infer<typeof wsErrorMessageSchema>;

export const wsPongMessageSchema = z.object({
  type: z.literal('pong'),
});

export type WsPongMessage = z.infer<typeof wsPongMessageSchema>;

export const wsServerMessageSchema = z.discriminatedUnion('type', [
  wsStartMessageSchema,
  wsTokenMessageSchema,
  wsPlanMessageSchema,
  wsCompleteMessageSchema,
  wsErrorMessageSchema,
  wsPongMessageSchema,
]);

export type WsServerMessage = z.infer<typeof wsServerMessageSchema>;

export interface WsExecuteMessage {
  type: 'execute';
  prompt: string;
  mode: string;
  model?: string;
  session_id?: string;
}

export interface WsCancelMessage {
  type: 'cancel';
}

export interface WsPingMessage {
  type: 'ping';
}

export type WsClientMessage = WsExecuteMessage | WsCancelMessage | WsPingMessage;

// ============================================================================
// SESSIONS
// ============================================================================

export const sessionSummarySchema = z.object({
  id: z.string(),
  title: z.string(),
  created_at: z.string(),
  message_count: z.number(),
});

export type SessionSummary = z.infer<typeof sessionSummarySchema>;

export const sessionsListSchema = z.array(sessionSummarySchema);
export type SessionsList = z.infer<typeof sessionsListSchema>;

export const sessionSchema = z.object({
  id: z.string(),
  title: z.string(),
  created_at: z.string(),
  messages: z.array(z.object({
    id: z.string(),
    role: z.string(),
    content: z.string(),
    model: z.string().optional().nullable(),
    timestamp: z.string(),
    agent: z.string().optional().nullable(),
  })),
});

export type Session = z.infer<typeof sessionSchema>;
