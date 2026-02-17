// src/features/agents/hooks/useAgents.ts
/**
 * GeminiHydra v15 - Agent Hooks
 * ==============================
 * TanStack Query hooks for agent listing, classification, and execution.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiDelete, apiGet, apiPost } from '@/shared/api/client';
import type { Agent, AgentsList, ClassifyResponse, ExecuteResponse } from '@/shared/api/schemas';

export function useAgentsQuery() {
  return useQuery<AgentsList>({
    queryKey: ['agents'],
    queryFn: () => apiGet<AgentsList>('/api/agents'),
  });
}

export function useCreateAgentMutation() {
  const queryClient = useQueryClient();
  return useMutation<void, Error, Agent>({
    mutationFn: (agent) => apiPost<void>('/api/agents', agent),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['agents'] });
    },
  });
}

export function useUpdateAgentMutation() {
  const queryClient = useQueryClient();
  return useMutation<void, Error, { id: string; agent: Agent }>({
    mutationFn: ({ id, agent }) => apiPost<void>(`/api/agents/${id}`, agent),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['agents'] });
    },
  });
}

export function useDeleteAgentMutation() {
  const queryClient = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (id) => apiDelete<void>(`/api/agents/${id}`),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['agents'] });
    },
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
