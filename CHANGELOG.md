# 15.0.0 (2026-02-25)


### Bug Fixes

* AI agents now use tools instead of RPG monologues ([9de0be4](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/9de0be4f4cfafcacf5cc0b5275c18dfea3b46919))
* **backend:** agents now know they have local filesystem access ([525ad73](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/525ad73046eed016ca81c72d2d2a24d77ff7c6de))
* **backend:** correct Gemini API tool declaration field name (snake_case) & config: enable flexible CORS and backend URL for Vercel/Fly deployment ([d7317a0](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/d7317a04d405c57cb2b6aa00a17b37667b5fbb2d))
* **backend:** correct model list for 2026 (Gemini 3) & set default to Flash Thinking ([d3140ab](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/d3140aba28a1ea06782bd2341b44ec7b541379d1))
* **backend:** make migrations idempotent and pgvector-optional ([c8a3153](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/c8a315352ad44c9eac416f4b72fed17e9a54aa95))
* **backend:** unique migration timestamps and axum 0.8 route syntax ([85d004e](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/85d004e754587376402bc2f6908b403682c7eca2))
* **backend:** update tree-sitter to v0.24+ and ring to 0.17.14 ([1d3bd86](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/1d3bd8671bb922af33ec9d663e901aa392c546c5))
* detect directory paths in prompts and load their contents as context ([efe438d](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/efe438d44fce6b1c91b992ca8f75b4783e1a4590))
* **frontend:** update WebSocket URL construction to respect VITE_BACKEND_URL for production deployments ([bb1a0b4](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/bb1a0b4cb23bf69e3af7ad377966a941d32ddde3))
* HTTP fallback now correctly activates when WebSocket is unavailable ([1a5476e](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/1a5476e6d917c9d7a231966335ea821b1311b81e))
* **i18n:** complete translations — fix Polish defaults, add missing keys ([1072e27](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/1072e27024bcb7aa50c141e26184c6d5975cf8db))
* inject model name into system prompt and wire live data to StatusFooter ([a3dcaee](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/a3dcaeeae11123c07a06e4e0457e5e49a5610b82))
* inject user language preference from DB settings into system prompt ([ef8fa40](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/ef8fa40c5b779a5d3dcc618ed84776fe45a9a60b))
* **migrations:** normalize line endings CRLF → LF and add migration 010 ([d1445a0](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/d1445a000c6e82594223c40ccdaa4c385e946abf))
* **partner:** portal escape for PartnerChatModal + UI polish ([9b0ad4a](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/9b0ad4af204cf92533784999356c934e401b5200))
* prevent heartbeat from killing WebSocket during tool-call streaming ([a7ad155](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/a7ad1555ad4712b918509890f1654cf48b085bfc))
* resolve Biome lint errors and improve code quality ([414b94b](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/414b94b771081832c4ca4845e7a52f1dc606d3fd))
* restore Polish diacritics in pl.json and fix trailing comma in en.json ([c476a6c](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/c476a6cd1c45054a65e939bd5cb37e78799e0429))
* UI fixes — tab bar flicker, Ctrl+Enter newline, native file picker, sidebar cleanup ([5e930c7](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/5e930c7b7f5b8ad43b53ac197b22a7914e286511))


### Features

* add Fly.io backend deployment + production API URLs ([7d975ad](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/7d975ad6a7050616b4430b3d497dbf7343f45758))
* add optional auth middleware (AUTH_SECRET Bearer token) ([75dba04](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/75dba0430f39e3dfaadaf02bd3510ec9c43dff85))
* add readiness probe, watchdog, dynamic CSS vars, and Polish default ([0ec3529](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/0ec35295cb573cb67226c70fae3e4c68c6aab100))
* add WebSocket streaming for real-time AI responses ([25f5b6e](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/25f5b6e0c0caca1ebb4b7e12e75880f0e88bba29))
* add welcome message, file tools, and fix Gemini thought_signature ([f19571f](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/f19571f470f7b30c50a456ba8d6b86bc024f8d08))
* **backend+frontend:** add Anthropic OAuth (MAX Plan) integration ([f273d77](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/f273d77dc8a9d0d4d2fdfcc45d049a901df9c589))
* **backend:** add Gemini 2.0 Flash Thinking model support ([e0614fa](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/e0614fa6c3e0a20744abedee7f5ef97599a73bce))
* **backend:** auto-fetch models at startup and set latest Pro as default ([a221267](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/a22126735bd3b739ae497bfd9a49dc3a4918be4c))
* **backend:** background system monitor with cached snapshots ([747adef](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/747adefdbd8397d6ce34595058279a14280e46fe))
* **chat:** per-session streaming — unblock input on inactive tabs ([e246002](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/e246002e0345fb380897771ee0910dc3f3cd016e))
* GeminiHydra v15.0.0 - Multi-Agent AI Swarm ([e059aa7](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/e059aa7690e01f94ee8ff3f8e75e49f63dd44807))
* **i18n:** translate HomeView — add home and time sections to en/pl JSON, convert WelcomeScreen.tsx to use useTranslation ([9106179](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/9106179c36447c9943ad44be805011fcf575bbe5))
* migrate backend to shared Postgres (sqlx 0.8) ([38e60de](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/38e60de46067e986d3d80b10a2905bc28b44c72a))
* **migrations:** add model pins migration (010) ([c0845cf](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/c0845cf54d854b818ea1b48d71c8d17851c9fe87))
* non-blocking startup, API retry, and backend health hook ([70308bb](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/70308bbe666742fabab0a3788963d5de6f9bb0d2))
* **partner:** cross-session visibility with ClaudeHydra ([ee468b3](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/ee468b3f7e853843037202884f728a317264a37e))
* restore Agents/Brain features without sidebar navigation ([10c82f3](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/10c82f35809b28cabf1429455a9ce20f76523bd7))
* security hardening, dead code cleanup, i18n completeness, logo optimization ([987c73e](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/987c73e4a207085c6a73187ff0cef671e75c5b6a))
* **sidebar:** align with Tissaia design — glass panel sessions, simplified nav ([7983128](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/798312810fdfa7e5d7c1b1ce312c59d3beb37ef8))
* unit tests, health dashboard, PWA, dead export cleanup, DX improvements ([30d715f](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/30d715f5c51e594bd0d4aa35f0e738c4f33a11da))


### Performance Improvements

* vendor chunk splitting + selective highlight.js + zod extraction ([9bbebcd](https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15/commit/9bbebcdd0bfde154c01f2d84e658efb2b0cb4c9d))



