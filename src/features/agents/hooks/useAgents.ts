// src/features/agents/hooks/useAgents.ts
/**
 * GeminiHydra v15 - Agent Hooks
 * ==============================
 * TanStack Query hooks for agent listing, classification, and execution.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { apiDelete, apiGet, apiPost } from '@/shared/api/client';
import type { Agent, AgentsList } from '@/shared/api/schemas';

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
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
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
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
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
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Operation failed');
    },
  });
}
