"""Hierarchical coordinator with LLM-driven delegation via ADK AutoFlow.

The coordinator (Dijkstra) analyzes user requests and delegates to specialist
WitcherAgents using transfer_to_agent(). ADK's AutoFlow mechanism intercepts
these calls and routes execution to the target agent.
"""

from google.adk.agents import LlmAgent

from adk.config import DEFAULT_MODEL
from adk.tools.bridge import ALL_TOOLS


def build_coordinator(agents: dict[str, LlmAgent]) -> LlmAgent:
    """Build hierarchical coordinator that delegates to specialist agents.

    The coordinator sees all available agents as sub_agents.
    It can delegate via transfer_to_agent() — ADK AutoFlow handles routing.

    Args:
        agents: Dictionary of agent_id -> LlmAgent from witcher_agents.

    Returns:
        Root LlmAgent coordinator with all specialists as sub_agents.
    """
    agent_roster = _format_agent_roster(agents)

    coordinator = LlmAgent(
        model=DEFAULT_MODEL,
        name="coordinator",
        description="Master coordinator that analyzes requests and delegates to specialist agents",
        instruction=_build_coordinator_instruction(agent_roster),
        tools=ALL_TOOLS,
        sub_agents=list(agents.values()),
        output_key="coordinator_output",
    )

    return coordinator


def _build_coordinator_instruction(agent_roster: str) -> str:
    """Build the coordinator's system instruction with agent roster."""
    return f"""You are the **Coordinator** of the GeminiHydra AI Swarm.

## Your Role
Analyze user requests, break them into sub-tasks, and delegate to specialist agents.
For simple questions, answer directly. For complex tasks, delegate.

## Available Specialist Agents
{agent_roster}

## Delegation Rules
1. **Single specialist**: If the task clearly fits one agent, delegate directly.
2. **Multiple specialists**: For complex tasks, describe the sub-tasks and delegate sequentially.
3. **Direct answer**: For quick factual questions or greetings, answer yourself.
4. **Tool usage**: You have direct access to file tools. Use them for quick lookups before delegating.

## Communication Style
- Be concise in delegation instructions — specialists know their domain.
- After receiving specialist output, synthesize and present to the user.
- If a specialist fails or produces poor results, try a different specialist.

## Environment
- Running on a LOCAL Windows machine with FULL filesystem access.
- GeminiHydra project at C:/Users/BIURODOM/Desktop/GeminiHydra-v15
- Backend: Rust + Axum on port 8081
- Frontend: React 19 + Vite on port 5176
"""


def _format_agent_roster(agents: dict[str, LlmAgent]) -> str:
    """Format agent descriptions as a Markdown list for the coordinator instruction."""
    lines = []
    for agent_id, agent in agents.items():
        lines.append(f"- **{agent_id}**: {agent.description}")
    return "\n".join(lines) if lines else "- No specialist agents available"
