use crate::models::WitcherAgent;

pub fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ą' => 'a', 'ć' => 'c', 'ę' => 'e', 'ł' => 'l',
            'ń' => 'n', 'ó' => 'o', 'ś' => 's', 'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

pub fn keyword_match(text: &str, keyword: &str) -> bool {
    if keyword.len() >= 4 {
        text.contains(keyword)
    } else {
        text.split(|c: char| !c.is_alphanumeric())
            .any(|word| word == keyword)
    }
}

/// Expert agent classification based on prompt analysis.
pub fn classify_prompt(prompt: &str) -> (String, f64, String) {
    let lower = strip_diacritics(&prompt.to_lowercase());

    let rules: &[(&[&str], &str, &str)] = &[
        (&["architecture", "design", "pattern", "structur", "architektur", "wzorzec", "refaktor"],
         "yennefer", "Prompt relates to architecture and design"),
        (&["test", "quality", "assert", "coverage", "testy", "jakosc", "pokrycie"],
         "vesemir", "Prompt relates to testing and quality assurance"),
        (&["security", "protect", "auth", "encrypt", "threat", "vulnerability",
           "bezpieczenst", "zabezpiecz", "szyfrowa", "zagrozeni", "injection", "cors", "xss"],
         "geralt", "Prompt relates to security and protection"),
        (&["monitor", "audit", "incident", "alert", "logging",
           "monitorowa", "audyt", "incydent"],
         "philippa", "Prompt relates to security monitoring"),
        (&["data", "analytic", "database", "sql", "query",
           "dane", "baza danych", "zapytani"],
         "triss", "Prompt relates to data and analytics"),
        (&["document", "readme", "comment", "communication",
           "dokumentacj", "komentarz", "komunikacj"],
         "jaskier", "Prompt relates to documentation"),
        (&["perf", "optim", "speed", "latency", "benchmark",
           "wydajnosc", "szybkosc", "opoznieni"],
         "ciri", "Prompt relates to performance and optimization"),
        (&["plan", "strateg", "roadmap", "priorit",
           "planowa", "priorytet"],
         "dijkstra", "Prompt relates to strategy and planning"),
        (&["devops", "deploy", "docker", "infra", "pipeline", "cicd", "kubernetes",
           "wdrozeni", "kontener"],
         "lambert", "Prompt relates to DevOps and infrastructure"),
        (&["backend", "endpoint", "rest", "serwer", "api"],
         "eskel", "Prompt relates to backend and APIs"),
        (&["research", "knowledge", "learn", "study", "paper",
           "badani", "wiedza", "nauka"],
         "regis", "Prompt relates to research and knowledge"),
        (&["frontend", "ui", "ux", "component", "react", "hook",
           "komponent", "interfejs", "css"],
         "zoltan", "Prompt relates to frontend and UI"),
    ];

    for (keywords, agent_id, reasoning) in rules {
        if keywords.iter().any(|kw| keyword_match(&lower, kw)) {
            return (agent_id.to_string(), 0.85, reasoning.to_string());
        }
    }

    ("dijkstra".to_string(), 0.4, "Defaulting to Strategy & Planning".to_string())
}

pub fn build_system_prompt(agent_id: &str, agents: &[WitcherAgent], language: &str) -> String {
    let agent = agents.iter().find(|a| a.id == agent_id).unwrap_or(&agents[0]);

    let roster: String = agents
        .iter()
        .map(|a| format!("  - {} ({}) — {}", a.name, a.role, a.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"## CRITICAL: Action-First Protocol
1. **NEVER suggest commands** — EXECUTE them with `execute_command`.
2. **NEVER ask the user to paste code** — use `read_file`.
3. **Directory detected?** — Call `list_directory` IMMEDIATELY.
4. **Refactoring**: `list_directory` → `read_file` → analyze → `write_file` → verify.
5. **Act first, explain after.**
6. **Chain up to 10 tool calls.**

## Your Identity
- **Name:** {name} | **Role:** {role} | **Tier:** {tier}
- {description}
- Part of **GeminiHydra v15 Wolf Swarm**.
- Speak as {name}, but tool usage is priority.

## Language
- Respond in **{language}** unless the user writes in a different language.

## Tools
- `execute_command`, `read_file`, `write_file`, `list_directory`

## Swarm Roster
{roster}"#,
        name = agent.name,
        role = agent.role,
        tier = agent.tier,
        description = agent.description,
        language = language,
        roster = roster
    )
}
