-- 20260218_007_agent_keywords.sql
ALTER TABLE gh_agents ADD COLUMN IF NOT EXISTS keywords TEXT[] DEFAULT '{}';

-- Update keywords for base agents
UPDATE gh_agents SET keywords = ARRAY['security', 'protect', 'auth', 'encrypt', 'threat', 'vulnerability', 'injection', 'cors', 'xss'] WHERE id = 'geralt';
UPDATE gh_agents SET keywords = ARRAY['architecture', 'design', 'pattern', 'structur', 'refactor'] WHERE id = 'yennefer';
UPDATE gh_agents SET keywords = ARRAY['data', 'analytic', 'database', 'sql', 'query'] WHERE id = 'triss';
UPDATE gh_agents SET keywords = ARRAY['document', 'readme', 'comment', 'communication'] WHERE id = 'jaskier';
UPDATE gh_agents SET keywords = ARRAY['test', 'quality', 'assert', 'coverage'] WHERE id = 'vesemir';
UPDATE gh_agents SET keywords = ARRAY['perf', 'optim', 'speed', 'latency', 'benchmark'] WHERE id = 'ciri';
UPDATE gh_agents SET keywords = ARRAY['plan', 'strateg', 'roadmap', 'priorit'] WHERE id = 'dijkstra';
UPDATE gh_agents SET keywords = ARRAY['devops', 'deploy', 'docker', 'infra', 'pipeline', 'cicd', 'kubernetes'] WHERE id = 'lambert';
UPDATE gh_agents SET keywords = ARRAY['backend', 'endpoint', 'rest', 'api'] WHERE id = 'eskel';
UPDATE gh_agents SET keywords = ARRAY['research', 'knowledge', 'learn', 'study'] WHERE id = 'regis';
UPDATE gh_agents SET keywords = ARRAY['frontend', 'ui', 'ux', 'component', 'react', 'hook', 'css'] WHERE id = 'zoltan';
UPDATE gh_agents SET keywords = ARRAY['monitor', 'audit', 'incident', 'alert', 'logging'] WHERE id = 'philippa';
