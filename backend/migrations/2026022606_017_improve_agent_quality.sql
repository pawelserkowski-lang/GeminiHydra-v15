-- 20260227_017_improve_agent_quality.sql
-- 1) Add Polish keywords to agents for better PL prompt classification
-- 2) Update max_tokens default from 8192 to 16384

-- Yennefer (Architecture): add Polish stems + system design keywords
UPDATE gh_agents SET keywords = ARRAY[
    'architecture', 'design', 'pattern', 'structur', 'refactor',
    'architektur', 'mikroserw', 'monolit', 'skalowa', 'system design',
    'wzorce', 'projektow'
] WHERE id = 'yennefer';

-- Geralt (Security): add Polish security keywords
UPDATE gh_agents SET keywords = ARRAY[
    'security', 'protect', 'auth', 'encrypt', 'threat', 'vulnerability', 'injection', 'cors', 'xss',
    'bezpiecz', 'zabezpiecz', 'uwierzytelni', 'szyfrow', 'zagro'
] WHERE id = 'geralt';

-- Triss (Data): add Polish data keywords
UPDATE gh_agents SET keywords = ARRAY[
    'data', 'analytic', 'database', 'sql', 'query',
    'baza danych', 'zapytani', 'anali'
] WHERE id = 'triss';

-- Vesemir (Testing): add Polish testing keywords
UPDATE gh_agents SET keywords = ARRAY[
    'test', 'quality', 'assert', 'coverage',
    'jakosc', 'pokryci'
] WHERE id = 'vesemir';

-- Ciri (Performance): add Polish perf keywords
UPDATE gh_agents SET keywords = ARRAY[
    'perf', 'optim', 'speed', 'latency', 'benchmark',
    'wydajnosc', 'szybkosc', 'opoznieni'
] WHERE id = 'ciri';

-- Lambert (DevOps): add Polish devops keywords
UPDATE gh_agents SET keywords = ARRAY[
    'devops', 'deploy', 'docker', 'infra', 'pipeline', 'cicd', 'kubernetes',
    'wdrozen', 'infrastruktur', 'kontener'
] WHERE id = 'lambert';

-- Zoltan (Frontend): add Polish frontend keywords
UPDATE gh_agents SET keywords = ARRAY[
    'frontend', 'ui', 'ux', 'component', 'react', 'hook', 'css',
    'komponent', 'interfejs', 'widok'
] WHERE id = 'zoltan';

-- Eskel (Backend): add Polish backend keywords
UPDATE gh_agents SET keywords = ARRAY[
    'backend', 'endpoint', 'rest', 'api',
    'serwer', 'koncowk'
] WHERE id = 'eskel';

-- Dijkstra (Strategy): add Polish strategy keywords
UPDATE gh_agents SET keywords = ARRAY[
    'plan', 'strateg', 'roadmap', 'priorit',
    'priorytet', 'harmonogram'
] WHERE id = 'dijkstra';

-- Jaskier (Docs): add Polish docs keywords
UPDATE gh_agents SET keywords = ARRAY[
    'document', 'readme', 'comment', 'communication',
    'dokumentacj', 'komentarz', 'komunikacj'
] WHERE id = 'jaskier';

-- Regis (Research): add Polish research keywords
UPDATE gh_agents SET keywords = ARRAY[
    'research', 'knowledge', 'learn', 'study',
    'badani', 'wiedz', 'nauk'
] WHERE id = 'regis';

-- Philippa (Monitoring): add Polish monitoring keywords
UPDATE gh_agents SET keywords = ARRAY[
    'monitor', 'audit', 'incident', 'alert', 'logging',
    'audyt', 'incydent', 'logowani'
] WHERE id = 'philippa';

-- Bump max_tokens from 8192 to 16384 for richer responses
UPDATE gh_settings SET max_tokens = 16384 WHERE max_tokens = 8192;
