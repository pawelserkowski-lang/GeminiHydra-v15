"""FastAPI server for GeminiHydra ADK sidecar.

Starts the ADK API server with:
- WitcherAgents loaded from PostgreSQL gh_agents table
- Hierarchical coordinator as root agent
- All orchestration patterns available
- InMemorySessionService (stateless per-restart, DB persistence via Rust backend)
"""

import asyncio
import logging
import os

import asyncpg
from dotenv import load_dotenv
from fastapi import FastAPI
from google.adk.agents import LlmAgent
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService

from adk.agents.coordinator import build_coordinator
from adk.agents.orchestration import build_all_pipelines
from adk.agents.review_pipeline import build_review_pipeline, build_security_review
from adk.agents.witcher_agents import build_witcher_agents
from adk.config import DATABASE_URL, PORT

load_dotenv()
logger = logging.getLogger("geminihydra-adk")

# Global references set at startup
_runner: Runner | None = None
_session_service: InMemorySessionService | None = None
_agents: dict[str, LlmAgent] = {}
_pipelines: dict = {}

app = FastAPI(title="GeminiHydra ADK Sidecar", version="1.0.0")


@app.on_event("startup")
async def startup():
    """Load agents from DB and build coordinator + pipelines."""
    global _runner, _session_service, _agents, _pipelines

    logger.info("Connecting to PostgreSQL...")
    try:
        pool = await asyncpg.create_pool(DATABASE_URL, min_size=1, max_size=5)
    except Exception as e:
        logger.warning(f"DB connection failed ({e}), using standalone coordinator")
        pool = None

    if pool:
        logger.info("Loading WitcherAgents from gh_agents...")
        _agents = await build_witcher_agents(pool)
        logger.info(f"Loaded {len(_agents)} agents: {list(_agents.keys())}")
        await pool.close()
    else:
        _agents = {}

    if _agents:
        coordinator = build_coordinator(_agents)
        _pipelines = build_all_pipelines(_agents)
        _pipelines["review"] = build_review_pipeline(_agents)
        _pipelines["security"] = build_security_review(_agents)
        logger.info(f"Built pipelines: {list(_pipelines.keys())}")
    else:
        from adk.agent import root_agent
        coordinator = root_agent
        logger.info("No DB agents found, using standalone coordinator")

    _session_service = InMemorySessionService()
    _runner = Runner(
        agent=coordinator,
        app_name="geminihydra",
        session_service=_session_service,
    )
    logger.info("ADK Runner initialized")


@app.get("/list-apps")
async def list_apps():
    """List available agent applications (health check endpoint)."""
    return {
        "apps": ["geminihydra"],
        "agents": list(_agents.keys()),
        "pipelines": list(_pipelines.keys()),
        "status": "ready" if _runner else "initializing",
    }


@app.get("/health")
async def health():
    """Health check."""
    return {"status": "ok", "agents": len(_agents), "pipelines": len(_pipelines)}


@app.post("/run")
async def run_agent(request: dict):
    """Execute agent and return all events as JSON array."""
    if not _runner or not _session_service:
        return {"error": "ADK not initialized"}

    user_id = request.get("userId", "default")
    session_id = request.get("sessionId", "default")
    message_text = _extract_message(request)
    pattern = request.get("config", {}).get("pattern")

    # Select agent based on pattern
    agent_runner = _get_runner_for_pattern(pattern)

    session = await _session_service.get_session(
        app_name="geminihydra", user_id=user_id, session_id=session_id
    )
    if not session:
        session = await _session_service.create_session(
            app_name="geminihydra", user_id=user_id, session_id=session_id
        )

    from google.genai import types
    user_content = types.Content(
        role="user", parts=[types.Part.from_text(text=message_text)]
    )

    events = []
    async for event in agent_runner.run_async(
        user_id=user_id, session_id=session_id, new_message=user_content
    ):
        events.append({
            "author": event.author if hasattr(event, "author") else "unknown",
            "content": str(event.content) if hasattr(event, "content") else "",
            "actions": str(event.actions) if hasattr(event, "actions") else None,
        })

    return {"events": events, "session_id": session_id}


@app.post("/run_sse")
async def run_agent_sse(request: dict):
    """Execute agent and stream events via SSE."""
    from sse_starlette.sse import EventSourceResponse

    if not _runner or not _session_service:
        async def error_gen():
            yield {"data": '{"error": "ADK not initialized"}'}
        return EventSourceResponse(error_gen())

    user_id = request.get("userId", "default")
    session_id = request.get("sessionId", "default")
    message_text = _extract_message(request)
    pattern = request.get("config", {}).get("pattern")

    agent_runner = _get_runner_for_pattern(pattern)

    session = await _session_service.get_session(
        app_name="geminihydra", user_id=user_id, session_id=session_id
    )
    if not session:
        session = await _session_service.create_session(
            app_name="geminihydra", user_id=user_id, session_id=session_id
        )

    from google.genai import types
    import json

    user_content = types.Content(
        role="user", parts=[types.Part.from_text(text=message_text)]
    )

    async def event_generator():
        async for event in agent_runner.run_async(
            user_id=user_id, session_id=session_id, new_message=user_content
        ):
            event_data = {
                "author": event.author if hasattr(event, "author") else "unknown",
                "timestamp": event.timestamp if hasattr(event, "timestamp") else None,
            }

            if hasattr(event, "content") and event.content:
                content = event.content
                if hasattr(content, "parts"):
                    text_parts = []
                    for part in content.parts:
                        if hasattr(part, "text") and part.text:
                            text_parts.append(part.text)
                        elif hasattr(part, "function_call"):
                            event_data["function_call"] = {
                                "name": part.function_call.name,
                                "args": dict(part.function_call.args) if part.function_call.args else {},
                            }
                        elif hasattr(part, "function_response"):
                            event_data["function_response"] = {
                                "name": part.function_response.name,
                            }
                    if text_parts:
                        event_data["text"] = "\n".join(text_parts)

                if hasattr(content, "role"):
                    event_data["role"] = content.role

            if hasattr(event, "actions") and event.actions:
                actions = event.actions
                if hasattr(actions, "escalate") and actions.escalate:
                    event_data["escalate"] = True
                if hasattr(actions, "transfer_to_agent") and actions.transfer_to_agent:
                    event_data["transfer_to_agent"] = actions.transfer_to_agent

            yield {"data": json.dumps(event_data)}

    return EventSourceResponse(event_generator())


def _extract_message(request: dict) -> str:
    """Extract user message text from ADK request format."""
    new_message = request.get("newMessage", {})
    parts = new_message.get("parts", [])
    for part in parts:
        if "text" in part:
            return part["text"]
    return request.get("prompt", "")


def _get_runner_for_pattern(pattern: str | None) -> Runner:
    """Get the appropriate Runner for the orchestration pattern."""
    global _runner, _session_service, _pipelines

    if not pattern or pattern == "hierarchical" or pattern not in _pipelines:
        return _runner

    pipeline_agent = _pipelines[pattern]
    return Runner(
        agent=pipeline_agent,
        app_name="geminihydra",
        session_service=_session_service,
    )


def main():
    """Entry point for running the server directly."""
    import uvicorn
    uvicorn.run("adk.server:app", host="0.0.0.0", port=PORT, reload=True)


if __name__ == "__main__":
    main()
