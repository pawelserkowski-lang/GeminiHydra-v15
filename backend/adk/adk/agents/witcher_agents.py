"""Load WitcherAgent definitions from PostgreSQL and build ADK LlmAgent instances.

Reads the gh_agents table at startup and creates one LlmAgent per active agent.
Each agent gets the full set of bridge tools and a unique output_key for state passing.
"""

import asyncpg
from google.adk.agents import LlmAgent

from adk.config import DEFAULT_MODEL
from adk.tools.bridge import ALL_TOOLS


async def build_witcher_agents(pool: asyncpg.Pool) -> dict[str, LlmAgent]:
    """Load agents from gh_agents DB and build ADK LlmAgent instances.

    Args:
        pool: asyncpg connection pool to the GeminiHydra database.

    Returns:
        Dictionary mapping agent_id -> LlmAgent instance.
    """
    rows = await pool.fetch(
        "SELECT id, name, role, tier, status, description, system_prompt, "
        "keywords, temperature, model_override "
        "FROM gh_agents WHERE status = 'online' ORDER BY created_at"
    )

    agents: dict[str, LlmAgent] = {}
    for row in rows:
        agent_id = row["id"]
        instruction = _build_instruction(row)
        model = row["model_override"] or DEFAULT_MODEL

        agent = LlmAgent(
            model=model,
            name=agent_id,
            description=f"{row['name']} ({row['role']}) — {row['description']}",
            instruction=instruction,
            tools=ALL_TOOLS,
            output_key=f"{agent_id}_output",
        )
        agents[agent_id] = agent

    return agents


def _build_instruction(row: asyncpg.Record) -> str:
    """Build agent instruction from DB row fields."""
    parts = [
        f"You are **{row['name']}**, a {row['role']} specialist in the GeminiHydra AI Swarm.",
        f"Tier: {row['tier']}.",
        "",
        f"Your domain: {row['description']}",
        "",
        "## Rules",
        "- You run on a LOCAL Windows machine with FULL filesystem access.",
        "- Use dedicated tools (list_directory, read_file, search_files, get_code_structure) for file operations.",
        "- NEVER use execute_command for reading, listing, searching, or analyzing files.",
        "- Call get_code_structure BEFORE read_file on source code files.",
        "- Request MULTIPLE tool calls in PARALLEL when they are independent.",
        "- Synthesize tool output into structured tables/lists.",
        "- Stop after 3-5 tool calls and write your analysis.",
    ]

    if row["system_prompt"]:
        parts.extend(["", "## Agent-Specific Instructions", row["system_prompt"]])

    return "\n".join(parts)
