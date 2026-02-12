use std::collections::HashMap;
use std::time::Instant;

use reqwest::Client;

use crate::models::{AppSettings, ChatMessage, WitcherAgent};
use crate::sessions::{KnowledgeEdge, KnowledgeNode, MemoryEntry};

pub struct AppState {
    pub settings: AppSettings,
    pub agents: Vec<WitcherAgent>,
    pub history: Vec<ChatMessage>,
    pub api_keys: HashMap<String, String>,
    pub start_time: Instant,
    pub client: Client,

    // --- memory / knowledge graph (sessions.rs) ---
    pub memories: Vec<MemoryEntry>,
    pub graph_nodes: Vec<KnowledgeNode>,
    pub graph_edges: Vec<KnowledgeEdge>,
}

impl AppState {
    pub fn new() -> Self {
        // ── API keys from environment ──────────────────────────────────
        let mut api_keys = HashMap::new();

        // GOOGLE_API_KEY takes precedence; fall back to GEMINI_API_KEY
        if let Ok(key) = std::env::var("GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
        {
            api_keys.insert("google".to_string(), key);
        }

        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            api_keys.insert("anthropic".to_string(), key);
        }

        // ── 12 Witcher agents ──────────────────────────────────────────
        let agents = vec![
            WitcherAgent {
                id: "geralt".into(),
                name: "Geralt".into(),
                role: "Security & Protection".into(),
                tier: "Commander".into(),
                status: "online".into(),
                description: "The White Wolf — leads security strategy, threat analysis, and protective measures across the swarm.".into(),
            },
            WitcherAgent {
                id: "yennefer".into(),
                name: "Yennefer".into(),
                role: "Architecture & Design".into(),
                tier: "Commander".into(),
                status: "online".into(),
                description: "The Sorceress of Vengerberg — designs system architecture, patterns, and high-level technical decisions.".into(),
            },
            WitcherAgent {
                id: "triss".into(),
                name: "Triss".into(),
                role: "Data & Analytics".into(),
                tier: "Coordinator".into(),
                status: "online".into(),
                description: "The Merigold — coordinates data pipelines, analytics, and insight extraction.".into(),
            },
            WitcherAgent {
                id: "jaskier".into(),
                name: "Jaskier".into(),
                role: "Documentation & Communication".into(),
                tier: "Coordinator".into(),
                status: "online".into(),
                description: "The Bard — manages documentation, communication, and knowledge sharing.".into(),
            },
            WitcherAgent {
                id: "vesemir".into(),
                name: "Vesemir".into(),
                role: "Testing & Quality".into(),
                tier: "Commander".into(),
                status: "online".into(),
                description: "The Elder Witcher — oversees testing strategy, quality assurance, and code reviews.".into(),
            },
            WitcherAgent {
                id: "ciri".into(),
                name: "Ciri".into(),
                role: "Performance & Optimization".into(),
                tier: "Coordinator".into(),
                status: "online".into(),
                description: "The Lion Cub of Cintra — coordinates performance profiling, optimization, and benchmarking.".into(),
            },
            WitcherAgent {
                id: "dijkstra".into(),
                name: "Dijkstra".into(),
                role: "Strategy & Planning".into(),
                tier: "Coordinator".into(),
                status: "online".into(),
                description: "The Spymaster — plans project strategy, roadmaps, and task prioritization.".into(),
            },
            WitcherAgent {
                id: "lambert".into(),
                name: "Lambert".into(),
                role: "DevOps & Infrastructure".into(),
                tier: "Executor".into(),
                status: "online".into(),
                description: "The Hothead — executes DevOps tasks, CI/CD pipelines, and infrastructure management.".into(),
            },
            WitcherAgent {
                id: "eskel".into(),
                name: "Eskel".into(),
                role: "Backend & APIs".into(),
                tier: "Executor".into(),
                status: "online".into(),
                description: "The Reliable — builds backend services, REST APIs, and server-side logic.".into(),
            },
            WitcherAgent {
                id: "regis".into(),
                name: "Regis".into(),
                role: "Research & Knowledge".into(),
                tier: "Executor".into(),
                status: "online".into(),
                description: "The Higher Vampire — conducts research, knowledge synthesis, and deep analysis.".into(),
            },
            WitcherAgent {
                id: "zoltan".into(),
                name: "Zoltan".into(),
                role: "Frontend & UI".into(),
                tier: "Executor".into(),
                status: "online".into(),
                description: "The Dwarf — builds frontend interfaces, UI components, and user experiences.".into(),
            },
            WitcherAgent {
                id: "philippa".into(),
                name: "Philippa".into(),
                role: "Security & Monitoring".into(),
                tier: "Executor".into(),
                status: "online".into(),
                description: "The Owl — executes security audits, monitoring, and incident response.".into(),
            },
        ];

        Self {
            settings: AppSettings::default(),
            agents,
            history: Vec::new(),
            api_keys,
            start_time: Instant::now(),
            client: Client::new(),
            memories: Vec::new(),
            graph_nodes: Vec::new(),
            graph_edges: Vec::new(),
        }
    }
}
