"""Orchestration patterns: Sequential, Parallel, and Loop agent compositions.

These patterns compose WitcherAgent LlmAgents into multi-agent workflows
using ADK's built-in workflow agents.
"""

from google.adk.agents import LlmAgent, SequentialAgent, ParallelAgent, LoopAgent, BaseAgent
from google.adk.events import Event, EventActions

from adk.config import DEFAULT_MODEL


# ---------------------------------------------------------------------------
# Pattern 1: Sequential Pipeline — Code Review
# ---------------------------------------------------------------------------

def build_code_review_pipeline(agents: dict[str, LlmAgent]) -> SequentialAgent:
    """Eskel analyzes -> Vesemir tests -> Geralt security -> Jaskier documents.

    Each agent reads the previous agent's output via session state (output_key).
    """
    sub = _pick_agents(agents, ["eskel", "vesemir", "geralt", "jaskier"])
    if len(sub) < 2:
        sub = list(agents.values())[:4]

    return SequentialAgent(
        name="code_review_pipeline",
        description="Full code review: analysis -> tests -> security -> documentation",
        sub_agents=sub,
    )


# ---------------------------------------------------------------------------
# Pattern 2: Parallel Analysis — Multi-Perspective
# ---------------------------------------------------------------------------

def build_analysis_pipeline(agents: dict[str, LlmAgent]) -> SequentialAgent:
    """Parallel analysis from 3 perspectives, then synthesis.

    Geralt (security), Ciri (performance), Yennefer (architecture) run concurrently.
    A synthesizer LlmAgent merges their findings.
    """
    parallel_agents = _pick_agents(agents, ["geralt", "ciri", "yennefer"])
    if len(parallel_agents) < 2:
        parallel_agents = list(agents.values())[:3]

    parallel = ParallelAgent(
        name="multi_analysis",
        description="Concurrent analysis from security, performance, and architecture perspectives",
        sub_agents=parallel_agents,
    )

    synthesizer = LlmAgent(
        model=DEFAULT_MODEL,
        name="synthesizer",
        description="Merges multi-perspective analysis into unified recommendations",
        instruction=(
            "You are the synthesis agent. Read the outputs from all analysis agents "
            "in the session state and produce a unified recommendation.\n\n"
            "For each perspective, summarize key findings and identify conflicts.\n"
            "Resolve conflicts with clear reasoning.\n"
            "Output a structured report with: Summary, Key Findings, Conflicts, Recommendations."
        ),
        output_key="synthesis_output",
    )

    return SequentialAgent(
        name="comprehensive_analysis",
        description="Parallel multi-perspective analysis followed by synthesis",
        sub_agents=[parallel, synthesizer],
    )


# ---------------------------------------------------------------------------
# Pattern 3: Loop — Iterative Refinement
# ---------------------------------------------------------------------------

class QualityGate(BaseAgent):
    """Checks if code quality meets threshold; exits loop if satisfied.

    Reads session state 'review_status' (set by reviewer agent).
    Escalates (exits loop) when status is 'approved' or max iterations reached.
    """

    def __init__(self, max_retries: int = 3):
        super().__init__(name="quality_gate", description="Quality gate that exits loop when approved")
        self._max_retries = max_retries

    async def _run_async_impl(self, ctx):
        review_status = ctx.session.state.get("review_status", "needs_work")
        iteration = ctx.session.state.get("iteration_count", 0)
        ctx.session.state["iteration_count"] = iteration + 1

        should_exit = review_status == "approved" or iteration >= self._max_retries

        status_msg = f"Quality gate: {review_status} (iteration {iteration + 1}/{self._max_retries + 1})"
        if should_exit and review_status != "approved":
            status_msg += " — max iterations reached, accepting current state"

        yield Event(
            author=self.name,
            actions=EventActions(escalate=should_exit),
            content=status_msg,
        )


def build_refinement_loop(agents: dict[str, LlmAgent]) -> LoopAgent:
    """Eskel codes -> reviewer reviews -> quality gate -> repeat if needed.

    The reviewer sets session state 'review_status' to 'approved' or 'needs_work'.
    QualityGate exits the loop when approved or after 3 retries.
    """
    coder = agents.get("eskel", list(agents.values())[0])

    reviewer = LlmAgent(
        model=DEFAULT_MODEL,
        name="code_reviewer",
        description="Reviews code for correctness, security, and style",
        instruction=(
            "Review the code from the previous agent's output. Check for:\n"
            "1. Correctness (logic errors, edge cases)\n"
            "2. Security (injection, auth bypass, unwrap in handlers)\n"
            "3. Performance (N+1 queries, missing indexes)\n"
            "4. Style (matches project patterns)\n\n"
            "If everything passes, set state: review_status = 'approved'\n"
            "If issues found, set state: review_status = 'needs_work' and list specific issues.\n"
            "Be concise — max 5 issues per review."
        ),
        output_key="review_output",
    )

    return LoopAgent(
        name="refinement_loop",
        description="Iterative code improvement until quality gate passes",
        max_iterations=5,
        sub_agents=[coder, reviewer, QualityGate()],
    )


# ---------------------------------------------------------------------------
# All preset orchestration pipelines
# ---------------------------------------------------------------------------

def build_all_pipelines(agents: dict[str, LlmAgent]) -> dict[str, SequentialAgent | LoopAgent]:
    """Build all preset orchestration pipelines from available agents."""
    return {
        "sequential": build_code_review_pipeline(agents),
        "parallel": build_analysis_pipeline(agents),
        "loop": build_refinement_loop(agents),
    }


def _pick_agents(agents: dict[str, LlmAgent], ids: list[str]) -> list[LlmAgent]:
    """Pick agents by ID, filtering out missing ones."""
    return [agents[aid] for aid in ids if aid in agents]
