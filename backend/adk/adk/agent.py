"""ADK root agent entry point.

This module defines root_agent — the entry point for the ADK server.
The coordinator acts as the root agent, delegating to WitcherAgent specialists.

For development, run: adk web
For production, run: adk api_server
"""

from google.adk.agents import LlmAgent

from adk.config import DEFAULT_MODEL
from adk.tools.bridge import ALL_TOOLS

# Default root agent for `adk web` / `adk api_server` CLI tools.
# This is a standalone coordinator without DB-loaded agents.
# The full DB-powered coordinator is built in server.py at startup.
root_agent = LlmAgent(
    model=DEFAULT_MODEL,
    name="coordinator",
    description="GeminiHydra AI Swarm Coordinator — delegates to specialist agents",
    instruction=(
        "You are the GeminiHydra AI Swarm Coordinator.\n"
        "You run on a LOCAL Windows machine with FULL filesystem access.\n"
        "Use the available tools to help users with software engineering tasks.\n"
        "Be concise and structured in your responses."
    ),
    tools=ALL_TOOLS,
    output_key="coordinator_output",
)
