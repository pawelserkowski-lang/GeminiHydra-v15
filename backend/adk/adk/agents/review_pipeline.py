"""Review/Critique pipeline: Generator produces -> Reviewer critiques.

Two-stage pattern where a generator agent creates a solution and a
specialized reviewer agent provides detailed critique with specific issues.
"""

from google.adk.agents import LlmAgent, SequentialAgent

from adk.config import DEFAULT_MODEL
from adk.tools.bridge import ALL_TOOLS


def build_review_pipeline(agents: dict[str, LlmAgent]) -> SequentialAgent:
    """Build generator -> reviewer sequential pipeline.

    The generator (typically Eskel) produces code/solutions.
    The critic reviews with specific criteria and outputs a structured report.

    Args:
        agents: Dictionary of agent_id -> LlmAgent.

    Returns:
        SequentialAgent with generator and critic.
    """
    generator = agents.get("eskel", list(agents.values())[0])

    critic = LlmAgent(
        model=DEFAULT_MODEL,
        name="critic",
        description="Code critic that reviews solutions for correctness, security, and style",
        instruction=(
            "You are a senior code reviewer. Review the solution from the previous agent.\n\n"
            "## Review Checklist\n"
            "1. **Correctness** — logic errors, edge cases, off-by-one\n"
            "2. **Security** — injection, auth bypass, .unwrap() in handlers, unsafe string slicing\n"
            "3. **Performance** — N+1 queries, missing indexes, unnecessary allocations\n"
            "4. **Style** — matches project patterns (Jaskier conventions)\n"
            "5. **Completeness** — does it fully address the original request?\n\n"
            "## Output Format\n"
            "- If clean: 'APPROVED — no issues found'\n"
            "- If issues: numbered list with file:line references and severity (critical/warning/info)\n\n"
            "Be specific — cite exact code. Max 10 issues."
        ),
        tools=ALL_TOOLS,
        output_key="review_result",
    )

    return SequentialAgent(
        name="review_critique",
        description="Generator produces solution, then critic reviews for quality",
        sub_agents=[generator, critic],
    )


def build_security_review(agents: dict[str, LlmAgent]) -> SequentialAgent:
    """Specialized security review pipeline.

    Any agent generates -> Geralt reviews for security specifically.
    """
    generator = agents.get("eskel", list(agents.values())[0])
    security = agents.get("geralt")

    if not security:
        security = LlmAgent(
            model=DEFAULT_MODEL,
            name="security_reviewer",
            description="Security specialist reviewer",
            instruction=(
                "You are a security specialist. Review the code for:\n"
                "- OWASP Top 10 vulnerabilities\n"
                "- Rust-specific: .unwrap() in handlers, unsafe blocks, unchecked input\n"
                "- SQL injection via format!() instead of parameterized queries\n"
                "- Auth bypass, missing middleware checks\n"
                "- Secrets in code, hardcoded credentials\n\n"
                "Output: severity + description + remediation for each finding."
            ),
            tools=ALL_TOOLS,
            output_key="security_review_result",
        )

    return SequentialAgent(
        name="security_review",
        description="Code generation followed by focused security review",
        sub_agents=[generator, security],
    )
