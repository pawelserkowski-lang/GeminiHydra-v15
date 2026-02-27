-- #29 — Expand agent keywords for better classification
-- Idempotent: UPDATE is safe to re-run (overwrites with same values)

UPDATE gh_agents SET keywords = ARRAY[
    'security', 'protect', 'auth', 'encrypt', 'threat', 'vulnerability', 'injection',
    'cors', 'xss', 'owasp', 'cve', 'credential', 'secret', 'token', 'leak', 'csrf',
    'sanitiz', 'privilege', 'permission'
] WHERE id = 'geralt';

UPDATE gh_agents SET keywords = ARRAY[
    'architecture', 'design', 'pattern', 'structur', 'refactor', 'abstraction',
    'coupling', 'cohesion', 'solid', 'separation', 'modulari', 'layered'
] WHERE id = 'yennefer';

UPDATE gh_agents SET keywords = ARRAY[
    'data', 'analytic', 'database', 'sql', 'query', 'migration', 'schema', 'postgres',
    'index', 'join', 'transaction', 'orm', 'normali', 'aggregate'
] WHERE id = 'triss';

UPDATE gh_agents SET keywords = ARRAY[
    'document', 'readme', 'comment', 'communication', 'changelog', 'jsdoc', 'rustdoc',
    'explain', 'describe', 'summarize', 'tutorial', 'guide'
] WHERE id = 'jaskier';

UPDATE gh_agents SET keywords = ARRAY[
    'test', 'quality', 'assert', 'coverage', 'spec', 'vitest', 'jest', 'playwright',
    'e2e', 'integration', 'unit', 'mock', 'fixture', 'snapshot'
] WHERE id = 'vesemir';

UPDATE gh_agents SET keywords = ARRAY[
    'perf', 'optim', 'speed', 'latency', 'benchmark', 'bundle', 'lighthouse',
    'lazy', 'cache', 'memo', 'profil', 'bottleneck', 'render'
] WHERE id = 'ciri';

UPDATE gh_agents SET keywords = ARRAY[
    'plan', 'strateg', 'roadmap', 'priorit', 'estimate', 'scope', 'tradeoff',
    'decision', 'compare', 'evaluate', 'assess', 'review'
] WHERE id = 'dijkstra';

UPDATE gh_agents SET keywords = ARRAY[
    'devops', 'deploy', 'docker', 'infra', 'pipeline', 'cicd', 'kubernetes',
    'nginx', 'terraform', 'github action', 'workflow', 'container', 'fly.io'
] WHERE id = 'lambert';

UPDATE gh_agents SET keywords = ARRAY[
    'backend', 'endpoint', 'rest', 'api', 'handler', 'middleware', 'route',
    'websocket', 'streaming', 'axum', 'reqwest', 'server', 'http', 'request'
] WHERE id = 'eskel';

UPDATE gh_agents SET keywords = ARRAY[
    'research', 'knowledge', 'learn', 'study', 'investigate', 'explore',
    'understand', 'analyze', 'deep dive', 'audit', 'inspect'
] WHERE id = 'regis';

UPDATE gh_agents SET keywords = ARRAY[
    'frontend', 'ui', 'ux', 'component', 'react', 'hook', 'css', 'tailwind',
    'animation', 'zustand', 'tanstack', 'vite', 'layout', 'responsive', 'mobile'
] WHERE id = 'zoltan';

UPDATE gh_agents SET keywords = ARRAY[
    'monitor', 'audit', 'incident', 'alert', 'logging', 'observ', 'metric',
    'tracing', 'dashboard', 'healthcheck', 'uptime', 'sentry'
] WHERE id = 'philippa';
