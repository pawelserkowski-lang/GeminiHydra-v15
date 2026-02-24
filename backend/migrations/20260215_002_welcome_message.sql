ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS welcome_message TEXT NOT NULL DEFAULT '';

UPDATE gh_settings SET welcome_message = '## 🐺 Witaj w GeminiHydra v15 — Wolf Swarm!

Jestem częścią **12-agentowego roju AI** opartego na Gemini. Każdy agent specjalizuje się w innej dziedzinie.

### 🔧 Dostępne narzędzia
Mogę **wykonywać akcje** na Twoim systemie — nie tylko sugerować komendy:
- **execute_command** — uruchamianie komend shell (build, test, git)
- **read_file** — odczyt plików z dysku
- **write_file** — tworzenie i nadpisywanie plików
- **list_directory** — listowanie zawartości katalogów

### 🗄️ Serwer SQL (PostgreSQL)
Backend połączony z bazą **PostgreSQL 17** (`geminihydra` na localhost:5432):
- `gh_settings` — konfiguracja aplikacji (model, język, temat, ta wiadomość)
- `gh_chat_messages` — historia konwersacji
- `gh_memories` — pamięć agentów
- `gh_knowledge_nodes/edges` — graf wiedzy

Napisz coś, np. *"wylistuj pliki na pulpicie"* lub *"uruchom cargo test w backendzie"* — a ja to **wykonam**!'
WHERE id = 1;
