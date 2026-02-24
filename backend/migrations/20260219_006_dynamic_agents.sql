-- 20260218_006_dynamic_agents.sql
CREATE TABLE IF NOT EXISTS gh_agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    tier TEXT NOT NULL,
    status TEXT NOT NULL,
    description TEXT NOT NULL,
    system_prompt TEXT, -- Optional custom prompt override
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed with the original Witcher Swarm if empty
INSERT INTO gh_agents (id, name, role, tier, status, description)
VALUES 
('geralt', 'Geralt', 'Security & Protection', 'Commander', 'online', 'The White Wolf — leads security strategy, threat analysis, and protective measures across the swarm.'),
('yennefer', 'Yennefer', 'Architecture & Design', 'Commander', 'online', 'The Sorceress of Vengerberg — designs system architecture, patterns, and high-level technical decisions.'),
('triss', 'Triss', 'Data & Analytics', 'Coordinator', 'online', 'The Merigold — coordinates data pipelines, analytics, and insight extraction.'),
('jaskier', 'Jaskier', 'Documentation & Communication', 'Coordinator', 'online', 'The Bard — manages documentation, communication, and knowledge sharing.'),
('vesemir', 'Vesemir', 'Testing & Quality', 'Commander', 'online', 'The Elder Witcher — oversees testing strategy, quality assurance, and code reviews.'),
('ciri', 'Ciri', 'Performance & Optimization', 'Coordinator', 'online', 'The Lion Cub of Cintra — coordinates performance profiling, optimization, and benchmarking.'),
('dijkstra', 'Dijkstra', 'Strategy & Planning', 'Coordinator', 'online', 'The Spymaster — plans project strategy, roadmaps, and task prioritization.'),
('lambert', 'Lambert', 'DevOps & Infrastructure', 'Executor', 'online', 'The Hothead — executes DevOps tasks, CI/CD pipelines, and infrastructure management.'),
('eskel', 'Eskel', 'Backend & APIs', 'Executor', 'online', 'The Reliable — builds backend services, REST APIs, and server-side logic.'),
('regis', 'Regis', 'Research & Knowledge', 'Executor', 'online', 'The Higher Vampire — conducts research, knowledge synthesis, and deep analysis.'),
('zoltan', 'Zoltan', 'Frontend & UI', 'Executor', 'online', 'The Dwarf — builds frontend interfaces, UI components, and user experiences.'),
('philippa', 'Philippa', 'Security & Monitoring', 'Executor', 'online', 'The Owl — executes security audits, monitoring, and incident response.')
ON CONFLICT (id) DO NOTHING;
