// ---------------------------------------------------------------------------
// handlers/tests.rs — Unit tests for classification, keyword matching, helpers
// ---------------------------------------------------------------------------

use super::*;
use crate::models::WitcherAgent;

/// Build a minimal set of test agents with keywords matching the DB seed.
fn test_agents() -> Vec<WitcherAgent> {
    vec![
        WitcherAgent {
            id: "yennefer".to_string(),
            name: "Yennefer".to_string(),
            role: "Architecture".to_string(),
            tier: "Commander".to_string(),
            status: "active".to_string(),
            description: "Architecture".to_string(),
            system_prompt: None,
            keywords: vec![
                "architecture".to_string(),
                "design".to_string(),
                "pattern".to_string(),
                "structur".to_string(),
                "refactor".to_string(),
            ],
            temperature: None,
            model_override: None,
            thinking_level: None,
            model_b: None,
            ab_split: None,
        },
        WitcherAgent {
            id: "triss".to_string(),
            name: "Triss".to_string(),
            role: "Data".to_string(),
            tier: "Coordinator".to_string(),
            status: "active".to_string(),
            description: "Data".to_string(),
            system_prompt: None,
            keywords: vec![
                "data".to_string(),
                "analytic".to_string(),
                "database".to_string(),
                "sql".to_string(),
                "query".to_string(),
            ],
            temperature: None,
            model_override: None,
            thinking_level: None,
            model_b: None,
            ab_split: None,
        },
        WitcherAgent {
            id: "dijkstra".to_string(),
            name: "Dijkstra".to_string(),
            role: "Strategy".to_string(),
            tier: "Coordinator".to_string(),
            status: "active".to_string(),
            description: "Strategy".to_string(),
            system_prompt: None,
            keywords: vec![
                "plan".to_string(),
                "strateg".to_string(),
                "roadmap".to_string(),
                "priorit".to_string(),
            ],
            temperature: None,
            model_override: None,
            thinking_level: None,
            model_b: None,
            ab_split: None,
        },
        WitcherAgent {
            id: "eskel".to_string(),
            name: "Eskel".to_string(),
            role: "Backend & APIs".to_string(),
            tier: "Coordinator".to_string(),
            status: "active".to_string(),
            description: "Backend & APIs".to_string(),
            system_prompt: None,
            keywords: vec![
                "backend".to_string(),
                "endpoint".to_string(),
                "rest".to_string(),
                "api".to_string(),
                "handler".to_string(),
                "middleware".to_string(),
                "route".to_string(),
                "websocket".to_string(),
            ],
            temperature: None,
            model_override: None,
            thinking_level: None,
            model_b: None,
            ab_split: None,
        },
    ]
}

#[test]
fn test_refactor_routes_to_yennefer() {
    let agents = test_agents();
    // "refactor this code" contains the keyword "refactor" (>= 4 chars → substring match)
    let (agent, confidence, _) = classify_prompt("refactor this code please", &agents);
    assert_eq!(agent, "yennefer");
    assert!(confidence >= 0.8);
}

#[test]
fn test_sql_routes_to_triss() {
    let agents = test_agents();
    let (agent, confidence, _) = classify_prompt("query sql database", &agents);
    assert_eq!(agent, "triss");
    assert!(confidence >= 0.8);
}

#[test]
fn test_unknown_prompt_falls_back_to_eskel() {
    let agents = test_agents();
    let (agent, _, _) = classify_prompt("what is the meaning of life", &agents);
    assert_eq!(agent, "eskel");
}

#[test]
fn test_backend_routes_to_eskel() {
    let agents = test_agents();
    let (agent, confidence, _) = classify_prompt("add a new api endpoint for user registration", &agents);
    assert_eq!(agent, "eskel");
    assert!(confidence >= 0.7);
}

#[test]
fn test_classify_agent_score_returns_zero_for_no_match() {
    let agents = test_agents();
    let score = classify_agent_score("nothing relevant here", &agents[0]);
    assert_eq!(score, 0.0);
}

#[test]
fn test_classify_agent_score_positive_for_match() {
    let agents = test_agents();
    let triss = &agents[1]; // triss has "sql", "database" etc.
    let score = classify_agent_score("query sql database migration", triss);
    assert!(score > 0.65);
}

#[test]
fn test_short_keyword_whole_word() {
    assert!(keyword_match("query sql database", "sql"));
    assert!(!keyword_match("results-only", "sql"));
}

#[test]
fn test_strip_diacritics_works() {
    assert_eq!(strip_diacritics("refaktoryzację"), "refaktoryzacje");
    assert_eq!(strip_diacritics("żółw"), "zolw");
}
