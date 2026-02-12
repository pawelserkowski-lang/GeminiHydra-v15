// src/features/agents/hooks/useAgents.ts
/**
 * GeminiHydra v15 - Agent Hooks
 * ==============================
 * TanStack Query hooks for agent listing, classification, and execution.
 */

import { useMutation, useQuery } from '@tanstack/react-query';
import { apiGet, apiPost } from '@/shared/api/client';
import type { AgentsList, ClassifyResponse, ExecuteResponse } from '@/shared/api/schemas';

export function useAgentsQuery() {
  return useQuery<AgentsList>({
    queryKey: ['agents'],
    queryFn: () => apiGet<AgentsList>('/api/agents'),
  });
}

interface ClassifyInput {
  message: string;
}

export function useClassifyMutation() {
  return useMutation<ClassifyResponse, Error, ClassifyInput>({
    mutationFn: (input) => apiPost<ClassifyResponse>('/api/agents/classify', input),
  });
}

interface ExecuteInput {
  message: string;
  agent_id?: string;
  model?: string;
  temperature?: number;
}

export function useExecuteMutation() {
  return useMutation<ExecuteResponse, Error, ExecuteInput>({
    mutationFn: (input) => apiPost<ExecuteResponse>('/api/execute', input),
  });
}
