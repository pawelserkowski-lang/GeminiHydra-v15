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

const healthSchema = z.object({
  status: z.string(),
  version: z.string(),
  app: z.string(),
  uptime_seconds: z.number(),
  providers: z.array(
    z.object({
      name: z.string(),
      available: z.boolean(),
      model: z.string().nullable().optional(),
    }),
  ),
});

export type Health = z.infer<typeof healthSchema>;

const detailedHealthSchema = healthSchema.extend({
  memory_usage_mb: z.number(),
  cpu_usage_percent: z.number(),
  platform: z.string(),
});

export type DetailedHealth = z.infer<typeof detailedHealthSchema>;

// ============================================================================
// AGENTS
// ============================================================================

const agentSchema = z.object({
  id: z.string(),
  name: z.string(),
  role: z.string(),
  tier: z.string(),
  status: z.string(),
  description: z.string(),
  system_prompt: z.string().optional(),
  keywords: z.array(z.string()).default([]),
});

export type Agent = z.infer<typeof agentSchema>;

const agentsListSchema = z.object({
  agents: z.array(agentSchema),
});

export type AgentsList = z.infer<typeof agentsListSchema>;

// ============================================================================
// EXECUTE
// ============================================================================

const executeResponseSchema = z.object({
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

const fileReadResponseSchema = z.object({
  path: z.string(),
  content: z.string(),
  size_bytes: z.number(),
  truncated: z.boolean(),
  extension: z.string(),
});

export type FileReadResponse = z.infer<typeof fileReadResponseSchema>;

// ============================================================================
// SYSTEM STATS
// ============================================================================

const systemStatsSchema = z.object({
  cpu_usage_percent: z.number(),
  memory_used_mb: z.number(),
  memory_total_mb: z.number(),
  platform: z.string(),
});

export type SystemStats = z.infer<typeof systemStatsSchema>;

// ============================================================================
// SETTINGS
// ============================================================================

const settingsSchema = z
  .object({
    temperature: z.number(),
    max_tokens: z.number(),
    default_model: z.string(),
    language: z.string(),
    theme: z.string(),
    welcome_message: z.string().optional().default(''),
    /** Gemini 3 thinking level — controls reasoning depth per request */
    thinking_level: z.enum(['none', 'minimal', 'low', 'medium', 'high']).optional().default('medium'),
    /** Max tool-call iterations per request (higher = more autonomous agent work) */
    max_iterations: z.number().optional().default(20),
    /** Working directory for filesystem tools (empty = absolute paths only) */
    working_directory: z.string().optional().default(''),
  })
  .passthrough();

export type Settings = z.infer<typeof settingsSchema>;

// ============================================================================
// AUTH STATUS (Google OAuth + API Key)
// ============================================================================

export const authStatusSchema = z.object({
  authenticated: z.boolean(),
  method: z.enum(['oauth', 'api_key', 'env']).optional(),
  expired: z.boolean().optional(),
  expires_at: z.number().optional(),
  user_email: z.string().optional(),
  user_name: z.string().optional(),
  oauth_available: z.boolean().optional(),
});

export type AuthStatus = z.infer<typeof authStatusSchema>;

/** @deprecated Use AuthStatus instead */
export type OAuthStatus = AuthStatus;

export const authLoginResponseSchema = z.object({
  auth_url: z.string(),
  state: z.string(),
});

export type AuthLoginResponse = z.infer<typeof authLoginResponseSchema>;

/** @deprecated Use AuthLoginResponse instead */
export type OAuthLoginResponse = AuthLoginResponse;

export const saveApiKeyResponseSchema = z.object({
  status: z.string(),
  authenticated: z.boolean(),
  valid: z.boolean(),
});

export type SaveApiKeyResponse = z.infer<typeof saveApiKeyResponseSchema>;

// ============================================================================
// WEBSOCKET PROTOCOL
// ============================================================================

const wsStartMessageSchema = z.object({
  type: z.literal('start'),
  id: z.string(),
  agent: z.string(),
  model: z.string(),
  files_loaded: z.array(z.string()),
});

export type WsStartMessage = z.infer<typeof wsStartMessageSchema>;

const wsTokenMessageSchema = z.object({
  type: z.literal('token'),
  content: z.string(),
});

const wsPlanMessageSchema = z.object({
  type: z.literal('plan'),
  agent: z.string(),
  confidence: z.number(),
  steps: z.array(z.string()),
});

export type WsPlanMessage = z.infer<typeof wsPlanMessageSchema>;

const wsCompleteMessageSchema = z.object({
  type: z.literal('complete'),
  duration_ms: z.number(),
});

export type WsCompleteMessage = z.infer<typeof wsCompleteMessageSchema>;

const wsErrorMessageSchema = z.object({
  type: z.literal('error'),
  message: z.string(),
  code: z.string().optional(),
});

const wsToolCallMessageSchema = z.object({
  type: z.literal('tool_call'),
  name: z.string(),
  args: z.unknown(),
  iteration: z.number(),
});

export type WsToolCallMessage = z.infer<typeof wsToolCallMessageSchema>;

const wsToolResultMessageSchema = z.object({
  type: z.literal('tool_result'),
  name: z.string(),
  success: z.boolean(),
  summary: z.string(),
  iteration: z.number(),
});

export type WsToolResultMessage = z.infer<typeof wsToolResultMessageSchema>;

const wsPongMessageSchema = z.object({
  type: z.literal('pong'),
});

// ── ADK Orchestration messages ──────────────────────────────────────────

const wsOrchestrationStartSchema = z.object({
  type: z.literal('orchestration_start'),
  pattern: z.string(),
  agents: z.array(z.string()),
});

export type WsOrchestrationStartMessage = z.infer<typeof wsOrchestrationStartSchema>;

const wsAgentDelegationSchema = z.object({
  type: z.literal('agent_delegation'),
  from_agent: z.string(),
  to_agent: z.string(),
  reason: z.string(),
});

export type WsAgentDelegationMessage = z.infer<typeof wsAgentDelegationSchema>;

const wsAgentOutputSchema = z.object({
  type: z.literal('agent_output'),
  agent: z.string(),
  content: z.string(),
  is_final: z.boolean(),
});

export type WsAgentOutputMessage = z.infer<typeof wsAgentOutputSchema>;

const wsPipelineProgressSchema = z.object({
  type: z.literal('pipeline_progress'),
  current_step: z.number(),
  total_steps: z.number(),
  current_agent: z.string(),
  status: z.string(),
});

export type WsPipelineProgressMessage = z.infer<typeof wsPipelineProgressSchema>;

const wsParallelStatusSchema = z.object({
  type: z.literal('parallel_status'),
  agents: z.array(
    z.object({
      agent: z.string(),
      status: z.string(),
      output_preview: z.string().optional().nullable(),
    }),
  ),
});

export type WsParallelStatusMessage = z.infer<typeof wsParallelStatusSchema>;

const wsHeartbeatSchema = z.object({
  type: z.literal('heartbeat'),
});

export const wsServerMessageSchema = z.discriminatedUnion('type', [
  wsStartMessageSchema,
  wsTokenMessageSchema,
  wsPlanMessageSchema,
  wsToolCallMessageSchema,
  wsToolResultMessageSchema,
  wsCompleteMessageSchema,
  wsErrorMessageSchema,
  wsPongMessageSchema,
  // ADK Orchestration
  wsOrchestrationStartSchema,
  wsAgentDelegationSchema,
  wsAgentOutputSchema,
  wsPipelineProgressSchema,
  wsParallelStatusSchema,
  wsHeartbeatSchema,
]);

export type WsServerMessage = z.infer<typeof wsServerMessageSchema>;

interface WsExecuteMessage {
  type: 'execute';
  prompt: string;
  mode: string;
  model?: string;
  session_id?: string;
}

interface WsOrchestrateMessage {
  type: 'orchestrate';
  prompt: string;
  pattern: string;
  agents?: string[];
  session_id?: string;
}

interface WsCancelMessage {
  type: 'cancel';
}

interface WsPingMessage {
  type: 'ping';
}

export type WsClientMessage = WsExecuteMessage | WsOrchestrateMessage | WsCancelMessage | WsPingMessage;

// ============================================================================
// SESSIONS
// ============================================================================

const sessionSummarySchema = z.object({
  id: z.string(),
  title: z.string(),
  created_at: z.string(),
  message_count: z.number(),
  working_directory: z.string().optional(),
  agent_id: z.string().nullable().optional(),
});

export type SessionSummary = z.infer<typeof sessionSummarySchema>;

const sessionsListSchema = z.array(sessionSummarySchema);
export type SessionsList = z.infer<typeof sessionsListSchema>;

const sessionSchema = z.object({
  id: z.string(),
  title: z.string(),
  created_at: z.string(),
  messages: z.array(
    z.object({
      id: z.string(),
      role: z.string(),
      content: z.string(),
      model: z.string().optional().nullable(),
      timestamp: z.string(),
      agent: z.string().optional().nullable(),
    }),
  ),
});

export type Session = z.infer<typeof sessionSchema>;

// ============================================================================
// OCR
// ============================================================================

export const ocrPageSchema = z.object({
  page_number: z.number(),
  text: z.string(),
});

export type OcrPage = z.infer<typeof ocrPageSchema>;

export const ocrResponseSchema = z.object({
  text: z.string(),
  pages: z.array(ocrPageSchema),
  total_pages: z.number(),
  processing_time_ms: z.number(),
  provider: z.string(),
  output_format: z.string().default('text'),
});

export type OcrResponse = z.infer<typeof ocrResponseSchema>;
