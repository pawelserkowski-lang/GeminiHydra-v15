// src/features/chat/hooks/useOrchestration.ts
/**
 * useOrchestration — ADK orchestration state management
 * ======================================================
 * Tracks multi-agent orchestration state: pattern, agent statuses,
 * pipeline progress, delegation chain. Fed by WS callbacks from
 * ChatViewWrapper.
 */

import { useCallback, useState } from 'react';

// ============================================================================
// TYPES
// ============================================================================

export type OrchestrationPattern = 'sequential' | 'parallel' | 'loop' | 'hierarchical' | 'review' | 'security';

export interface DelegationEvent {
  fromAgent: string;
  toAgent: string;
  reason: string;
  timestamp: number;
}

export interface AgentStatus {
  agent: string;
  status: 'pending' | 'running' | 'done' | 'error';
  outputPreview?: string;
}

export interface OrchestrationState {
  isOrchestrating: boolean;
  pattern: OrchestrationPattern | null;
  agents: AgentStatus[];
  currentStep: number;
  totalSteps: number;
  currentAgent: string | null;
  delegationChain: DelegationEvent[];
}

export const EMPTY_ORCHESTRATION: OrchestrationState = {
  isOrchestrating: false,
  pattern: null,
  agents: [],
  currentStep: 0,
  totalSteps: 0,
  currentAgent: null,
  delegationChain: [],
};

// ============================================================================
// HOOK
// ============================================================================

export function useOrchestration() {
  const [state, setState] = useState<OrchestrationState>(EMPTY_ORCHESTRATION);

  const onOrchestrationStart = useCallback((pattern: string, agents: string[]) => {
    setState({
      isOrchestrating: true,
      pattern: pattern as OrchestrationPattern,
      agents: agents.map((a) => ({ agent: a, status: 'pending' })),
      currentStep: 0,
      totalSteps: agents.length,
      currentAgent: agents[0] ?? null,
      delegationChain: [],
    });
  }, []);

  const onAgentDelegation = useCallback((fromAgent: string, toAgent: string, reason: string) => {
    setState((prev) => ({
      ...prev,
      currentAgent: toAgent,
      delegationChain: [...prev.delegationChain, { fromAgent, toAgent, reason, timestamp: Date.now() }],
      agents: prev.agents.map((a) => (a.agent === toAgent ? { ...a, status: 'running' } : a)),
    }));
  }, []);

  const onAgentOutput = useCallback((agent: string, content: string, isFinal: boolean) => {
    setState((prev) => ({
      ...prev,
      agents: prev.agents.map((a) =>
        a.agent === agent
          ? {
              ...a,
              status: isFinal ? 'done' : 'running',
              outputPreview: content.slice(0, 120),
            }
          : a,
      ),
    }));
  }, []);

  const onPipelineProgress = useCallback((currentStep: number, totalSteps: number, currentAgent: string) => {
    setState((prev) => ({
      ...prev,
      currentStep,
      totalSteps,
      currentAgent,
      agents: prev.agents.map((a) => {
        if (a.agent === currentAgent) return { ...a, status: 'running' };
        // Mark agents before current step as done
        const agentIdx = prev.agents.findIndex((x) => x.agent === a.agent);
        const currentIdx = prev.agents.findIndex((x) => x.agent === currentAgent);
        if (agentIdx < currentIdx && a.status !== 'done') return { ...a, status: 'done' };
        return a;
      }),
    }));
  }, []);

  const onParallelStatus = useCallback(
    (agents: Array<{ agent: string; status: string; output_preview?: string | null }>) => {
      setState((prev) => ({
        ...prev,
        agents: agents.map((a) => ({
          agent: a.agent,
          status: a.status === 'completed' ? 'done' : a.status === 'running' ? 'running' : 'pending',
          outputPreview: a.output_preview ?? undefined,
        })),
      }));
    },
    [],
  );

  const onComplete = useCallback(() => {
    setState((prev) => ({
      ...prev,
      isOrchestrating: false,
      agents: prev.agents.map((a) => (a.status === 'running' ? { ...a, status: 'done' } : a)),
    }));
  }, []);

  const reset = useCallback(() => {
    setState(EMPTY_ORCHESTRATION);
  }, []);

  return {
    orchestration: state,
    onOrchestrationStart,
    onAgentDelegation,
    onAgentOutput,
    onPipelineProgress,
    onParallelStatus,
    onOrchestrationComplete: onComplete,
    resetOrchestration: reset,
  };
}
