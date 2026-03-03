// ---------------------------------------------------------------------------
// tool_defs.rs — Gemini tool definitions (extracted from handlers/mod.rs)
// ---------------------------------------------------------------------------

use serde_json::{Value, json};

/// Tool definitions are static and never change — compute once via AppState OnceLock.
/// Byte-identical tools JSON across all requests enables Gemini implicit caching.
pub fn build_tools(state: &crate::state::AppState) -> Value {
    state.tool_defs_cache.get_or_init(|| json!([{
        "function_declarations": [
            {
                "name": "list_directory",
                "description": "List files and subdirectories in a local directory with sizes and line counts. ALWAYS use this to explore project structure — never use execute_command with dir/ls.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the local directory" }, "show_hidden": { "type": "boolean", "description": "Include hidden files (dotfiles)" } }, "required": ["path"] }
            },
            {
                "name": "read_file",
                "description": "Read a file from the local filesystem by its absolute path. ALWAYS use this to inspect code — never use execute_command with cat/type/Get-Content. For large files, use read_file_section instead.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the local file, e.g. C:\\Users\\...\\file.ts" } }, "required": ["path"] }
            },
            {
                "name": "read_file_section",
                "description": "Read specific line range from a file. Use AFTER get_code_structure to read only the functions you need — much cheaper than reading the entire file.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the file" }, "start_line": { "type": "integer", "description": "First line to read (1-indexed, inclusive)" }, "end_line": { "type": "integer", "description": "Last line to read (1-indexed, inclusive). Max range: 500 lines" } }, "required": ["path", "start_line", "end_line"] }
            },
            {
                "name": "search_files",
                "description": "Search for text or regex patterns across all files in a directory (recursive). Returns matching lines with file paths and line numbers. Supports pagination and multiline regex. ALWAYS use this to search for code patterns — never use execute_command with grep/Select-String/findstr.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Directory to search in (absolute path)" }, "pattern": { "type": "string", "description": "Text or regex pattern to search for (case-insensitive)" }, "file_extensions": { "type": "string", "description": "Comma-separated extensions to filter, e.g. 'ts,tsx,rs'. Default: all text files" }, "offset": { "type": "integer", "description": "Number of matches to skip (default 0, for pagination)" }, "limit": { "type": "integer", "description": "Max matches to return (default 80)" }, "multiline": { "type": "boolean", "description": "If true, pattern matches across line boundaries with ±2 lines context (default false)" } }, "required": ["path", "pattern"] }
            },
            {
                "name": "find_file",
                "description": "Find files by name pattern (glob). Returns matching file paths with sizes. Use when you don't know exact file location.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Root directory to search in (absolute path)" }, "pattern": { "type": "string", "description": "Glob pattern like '*.tsx' or 'auth*'" } }, "required": ["path", "pattern"] }
            },
            {
                "name": "get_code_structure",
                "description": "Analyze code structure (functions, classes, structs, traits) via AST without reading full file content. Returns symbol names, types, and line numbers. Supports Rust, TypeScript, JavaScript, Python, Go.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the source file to analyze" } }, "required": ["path"] }
            },
            {
                "name": "write_file",
                "description": "Write or create a file on the local filesystem. Use for creating NEW files or complete rewrites.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path for the file to write" }, "content": { "type": "string", "description": "Full file content to write" } }, "required": ["path", "content"] }
            },
            {
                "name": "edit_file",
                "description": "Edit an existing file by replacing a specific text section. SAFER than write_file — only changes the targeted section. CRITICAL: old_text must be COPIED VERBATIM from read_file output — every character, space, tab, and newline must match EXACTLY. Even one different space or missing newline causes failure. Use read_file_section first to get the exact text, then copy it character-for-character into old_text.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the file to edit" }, "old_text": { "type": "string", "description": "Text to find and replace — must be COPIED VERBATIM from the file (exact whitespace, exact newlines). Keep it short (3-10 lines) to minimize mismatch risk. Must appear exactly once in the file." }, "new_text": { "type": "string", "description": "Replacement text — same indentation style as the original" } }, "required": ["path", "old_text", "new_text"] }
            },
            {
                "name": "delete_file",
                "description": "Delete a file or empty directory from the local filesystem. IMPORTANT for Rust refactoring: when you create `foo/mod.rs`, you MUST immediately delete `foo.rs` — having both causes fatal E0761 compile error.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the file or empty directory to delete" } }, "required": ["path"] }
            },
            {
                "name": "diff_files",
                "description": "Compare two files and show line-by-line differences in unified diff format. Max 200 diff lines output.",
                "parameters": { "type": "object", "properties": { "path_a": { "type": "string", "description": "Absolute path to the first file" }, "path_b": { "type": "string", "description": "Absolute path to the second file" } }, "required": ["path_a", "path_b"] }
            },
            {
                "name": "read_pdf",
                "description": "Extract text from a PDF file. Uses pdf-extract for embedded text; falls back to Gemini Vision OCR for scanned/image-based PDFs. Supports page range filtering.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the PDF file" }, "page_range": { "type": "string", "description": "Optional page range like '1-5' or '3' (1-indexed)" } }, "required": ["path"] }
            },
            {
                "name": "analyze_image",
                "description": "Analyze an image file using Gemini Vision API. Describes contents, text, objects, colors, and notable features. Set extract_text=true to perform OCR (extract text from the image). Supports PNG, JPEG, WebP, GIF (max 10 MB).",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the image file" }, "prompt": { "type": "string", "description": "Optional custom analysis prompt" }, "extract_text": { "type": "boolean", "description": "When true, extract text (OCR) from the image instead of describing it" } }, "required": ["path"] }
            },
            {
                "name": "ocr_document",
                "description": "Extract text from an image or PDF using Gemini Vision OCR. Returns text with preserved formatting: tables as markdown (| pipes + --- separators), headers, lists, paragraphs. Ideal for invoices, reports, forms, tables, receipts, scanned documents. The extracted text can be copied with rich formatting (pastes as real tables in Word/Excel). Supports PNG, JPEG, WebP, GIF, PDF (max 22 MB).",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the image or PDF file" }, "prompt": { "type": "string", "description": "Optional custom OCR prompt (default extracts all text preserving tables and formatting)" } }, "required": ["path"] }
            },
            {
                "name": "fetch_webpage",
                "description": "Fetch a web page with SSRF protection, extract readable text (HTML tables→markdown, code→fenced blocks, inline links preserved), metadata (OpenGraph, JSON-LD, language), and categorized links (internal/external/resource). Supports retry with backoff, content deduplication, custom headers, and JSON output format.",
                "parameters": { "type": "object", "properties": {
                    "url": { "type": "string", "description": "Full URL to fetch (http/https). Private IPs and localhost are blocked." },
                    "extract_links": { "type": "boolean", "description": "Extract and categorize all links as internal/external/resource (default: true)" },
                    "extract_metadata": { "type": "boolean", "description": "Extract OpenGraph, JSON-LD, canonical URL, language (default: false)" },
                    "include_images": { "type": "boolean", "description": "Include image alt text as ![alt](src) in output (default: false)" },
                    "output_format": { "type": "string", "description": "Output format: 'text' (markdown) or 'json' (structured). Default: 'text'" },
                    "max_text_length": { "type": "integer", "description": "Max characters of page text to return. 0 = unlimited (default: 0)" },
                    "headers": { "type": "object", "description": "Custom HTTP headers as key-value pairs" }
                }, "required": ["url"] }
            },
            {
                "name": "crawl_website",
                "description": "Crawl a website with robots.txt compliance, optional sitemap seeding, concurrent requests, SSRF protection, and content deduplication. Extracts text from each page (tables→markdown, code→fenced) and builds categorized link index. Supports path prefix filtering, exclude patterns, and configurable rate limiting.",
                "parameters": { "type": "object", "properties": {
                    "url": { "type": "string", "description": "Starting URL to crawl (http/https)" },
                    "max_depth": { "type": "integer", "description": "Max link depth (default: 1, max: 5)" },
                    "max_pages": { "type": "integer", "description": "Max pages to fetch (default: 10, max: 50)" },
                    "same_domain_only": { "type": "boolean", "description": "Only follow same-domain links (default: true)" },
                    "path_prefix": { "type": "string", "description": "Only crawl URLs whose path starts with this prefix (e.g. '/docs/')" },
                    "exclude_patterns": { "type": "array", "items": { "type": "string" }, "description": "Skip URLs containing any of these substrings" },
                    "respect_robots_txt": { "type": "boolean", "description": "Fetch and respect robots.txt (default: true)" },
                    "use_sitemap": { "type": "boolean", "description": "Seed crawl queue from sitemap.xml (default: false)" },
                    "concurrent_requests": { "type": "integer", "description": "Concurrent fetches (default: 1, max: 5)" },
                    "delay_ms": { "type": "integer", "description": "Delay between requests in ms (default: 300)" },
                    "max_total_seconds": { "type": "integer", "description": "Max total crawl time in seconds (default: 180)" },
                    "output_format": { "type": "string", "description": "Output format: 'text' or 'json' (default: 'text')" },
                    "max_text_length": { "type": "integer", "description": "Max text chars per page excerpt (default: 2000)" },
                    "include_metadata": { "type": "boolean", "description": "Include OpenGraph/JSON-LD metadata per page (default: false)" },
                    "headers": { "type": "object", "description": "Custom HTTP headers as key-value pairs" }
                }, "required": ["url"] }
            },
            {
                "name": "git_status",
                "description": "Show working tree status (current branch, staged/unstaged changes, untracked files) for a git repository.",
                "parameters": { "type": "object", "properties": { "repo_path": { "type": "string", "description": "Absolute path to the git repository" } }, "required": ["repo_path"] }
            },
            {
                "name": "git_log",
                "description": "Show commit history as a graph with branch decorations. Returns up to 50 most recent commits.",
                "parameters": { "type": "object", "properties": { "repo_path": { "type": "string", "description": "Absolute path to the git repository" }, "count": { "type": "integer", "description": "Number of commits to show (default: 20, max: 50)" } }, "required": ["repo_path"] }
            },
            {
                "name": "git_diff",
                "description": "Show changes (diff) in a git repository. Use target='staged' for staged changes, or a commit/branch reference.",
                "parameters": { "type": "object", "properties": { "repo_path": { "type": "string", "description": "Absolute path to the git repository" }, "target": { "type": "string", "description": "What to diff: 'staged', '--stat', or a commit/branch reference (default: working tree --stat)" } }, "required": ["repo_path"] }
            },
            {
                "name": "git_branch",
                "description": "List, create, or switch git branches. Actions: 'list' (default), 'create:branch-name', 'switch:branch-name'.",
                "parameters": { "type": "object", "properties": { "repo_path": { "type": "string", "description": "Absolute path to the git repository" }, "action": { "type": "string", "description": "Branch action: 'list', 'create:name', or 'switch:name' (default: list)" } }, "required": ["repo_path"] }
            },
            {
                "name": "git_commit",
                "description": "Stage files and create a git commit. Does NOT push — only local commit. Use files='all' to stage everything, or comma-separated file list.",
                "parameters": { "type": "object", "properties": { "repo_path": { "type": "string", "description": "Absolute path to the git repository" }, "message": { "type": "string", "description": "Commit message" }, "files": { "type": "string", "description": "Files to stage: 'all' or comma-separated paths (e.g. 'src/main.rs,Cargo.toml'). If omitted, commits already-staged files." } }, "required": ["repo_path", "message"] }
            },
            {
                "name": "github_list_repos",
                "description": "List GitHub repositories for the authenticated user. Returns name, description, language, stars, and visibility. Requires GitHub OAuth.",
                "parameters": { "type": "object", "properties": { "sort": { "type": "string", "description": "Sort by: created, updated, pushed, full_name (default: updated)" }, "per_page": { "type": "integer", "description": "Results per page, max 100 (default: 30)" } }, "required": [] }
            },
            {
                "name": "github_get_repo",
                "description": "Get detailed information about a specific GitHub repository. Requires GitHub OAuth.",
                "parameters": { "type": "object", "properties": { "owner": { "type": "string", "description": "Repository owner (user or org)" }, "repo": { "type": "string", "description": "Repository name" } }, "required": ["owner", "repo"] }
            },
            {
                "name": "github_list_issues",
                "description": "List issues for a GitHub repository. Supports filtering by state (open/closed/all). Requires GitHub OAuth.",
                "parameters": { "type": "object", "properties": { "owner": { "type": "string", "description": "Repository owner" }, "repo": { "type": "string", "description": "Repository name" }, "state": { "type": "string", "description": "Filter by state: open, closed, all (default: open)" } }, "required": ["owner", "repo"] }
            },
            {
                "name": "github_get_issue",
                "description": "Get a specific GitHub issue with its comments. Requires GitHub OAuth.",
                "parameters": { "type": "object", "properties": { "owner": { "type": "string", "description": "Repository owner" }, "repo": { "type": "string", "description": "Repository name" }, "number": { "type": "integer", "description": "Issue number" } }, "required": ["owner", "repo", "number"] }
            },
            {
                "name": "github_create_issue",
                "description": "Create a new issue in a GitHub repository. Requires GitHub OAuth.",
                "parameters": { "type": "object", "properties": { "owner": { "type": "string", "description": "Repository owner" }, "repo": { "type": "string", "description": "Repository name" }, "title": { "type": "string", "description": "Issue title" }, "body": { "type": "string", "description": "Issue body (markdown)" } }, "required": ["owner", "repo", "title"] }
            },
            {
                "name": "github_create_pr",
                "description": "Create a pull request in a GitHub repository. Requires GitHub OAuth.",
                "parameters": { "type": "object", "properties": { "owner": { "type": "string", "description": "Repository owner" }, "repo": { "type": "string", "description": "Repository name" }, "title": { "type": "string", "description": "PR title" }, "body": { "type": "string", "description": "PR body (markdown)" }, "head": { "type": "string", "description": "Branch containing changes" }, "base": { "type": "string", "description": "Branch to merge into (default: main)" } }, "required": ["owner", "repo", "title", "head"] }
            },
            {
                "name": "vercel_list_projects",
                "description": "List Vercel projects for the authenticated user/team. Returns project names, frameworks, and latest deployments. Requires Vercel OAuth.",
                "parameters": { "type": "object", "properties": { "limit": { "type": "integer", "description": "Max results (default: 20, max: 100)" } }, "required": [] }
            },
            {
                "name": "vercel_get_deployment",
                "description": "Get details about a specific Vercel deployment by ID or URL. Requires Vercel OAuth.",
                "parameters": { "type": "object", "properties": { "deployment_id": { "type": "string", "description": "Deployment ID or URL" } }, "required": ["deployment_id"] }
            },
            {
                "name": "vercel_deploy",
                "description": "Trigger a new deployment for a Vercel project. Creates a deployment from the latest git commit. Requires Vercel OAuth.",
                "parameters": { "type": "object", "properties": { "project": { "type": "string", "description": "Project name or ID" }, "target": { "type": "string", "description": "Deployment target: production or preview (default: preview)" } }, "required": ["project"] }
            },
            {
                "name": "fly_list_apps",
                "description": "List Fly.io applications for the authenticated user. Returns app names, status, and organization. Requires Fly.io PAT (service token).",
                "parameters": { "type": "object", "properties": { "org_slug": { "type": "string", "description": "Organization slug to filter by (default: personal)" } }, "required": [] }
            },
            {
                "name": "fly_get_status",
                "description": "Get the status of a specific Fly.io application, including machine states, regions, and health checks. Requires Fly.io PAT.",
                "parameters": { "type": "object", "properties": { "app_name": { "type": "string", "description": "Name of the Fly.io application" } }, "required": ["app_name"] }
            },
            {
                "name": "fly_get_logs",
                "description": "Get recent logs for a Fly.io application with allocation details and release info. Requires Fly.io PAT.",
                "parameters": { "type": "object", "properties": { "app_name": { "type": "string", "description": "Name of the Fly.io application" } }, "required": ["app_name"] }
            },
            {
                "name": "list_zip",
                "description": "List contents of a ZIP archive (file names, sizes, compressed sizes). Max 100 MB archive.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the ZIP file" } }, "required": ["path"] }
            },
            {
                "name": "extract_zip_file",
                "description": "Extract and preview a single file from a ZIP archive. Returns text content or hex preview for binary files. Max 10 MB per file.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the ZIP archive" }, "file_path": { "type": "string", "description": "Path of the file inside the ZIP to extract" } }, "required": ["path", "file_path"] }
            },
            {
                "name": "call_agent",
                "description": "Delegate a subtask to another Witcher agent via A2A protocol. The target agent has full tool access and can read files, search code, etc. Use when the task requires specialized expertise (e.g., code analysis → Eskel, debugging → Lambert, data → Triss). Returns the agent's complete response. Max 3 delegation levels.",
                "parameters": { "type": "object", "properties": { "agent_id": { "type": "string", "description": "Target agent ID (e.g., 'eskel', 'lambert', 'triss', 'yennefer')" }, "task": { "type": "string", "description": "The subtask to delegate. Be specific about what you need and provide context." } }, "required": ["agent_id", "task"] }
            },
            {
                "name": "execute_command",
                "description": "Execute a shell command on the local Windows machine. ONLY use for build/test/git/npm/cargo CLI operations. NEVER use for file reading (use read_file), directory listing (use list_directory), or text search (use search_files). ALWAYS set working_directory when running project commands (cargo, npm, git).",
                "parameters": { "type": "object", "properties": { "command": { "type": "string", "description": "Shell command to execute (Windows cmd.exe). Do NOT include 'cd' — use working_directory instead." }, "working_directory": { "type": "string", "description": "Absolute path to set as the working directory before executing the command. REQUIRED for cargo/npm/git commands. Example: C:\\Users\\BIURODOM\\Desktop\\GeminiHydra-v15\\backend" } }, "required": ["command"] }
            },
            {
                "name": "list_mcp_tools",
                "description": "List all available MCP tools from connected external servers. Returns tool names, descriptions, and which server provides each tool.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            },
            {
                "name": "execute_mcp_tool",
                "description": "Execute a specific MCP tool by its prefixed name (mcp_servername_toolname) with custom arguments. Use list_mcp_tools first to discover available tools.",
                "parameters": { "type": "object", "properties": { "tool_name": { "type": "string", "description": "Prefixed MCP tool name (e.g. mcp_brave_web_search)" }, "arguments": { "type": "object", "description": "Tool arguments as a JSON object" } }, "required": ["tool_name"] }
            }
        ]
    }])).clone()
}

/// Build tools including dynamically discovered MCP tools.
/// Native tools are cached (OnceLock), MCP tools merged at request time.
/// MCP tools are placed FIRST — they are preferred over native equivalents.
pub async fn build_tools_with_mcp(state: &crate::state::AppState) -> serde_json::Value {
    let native = build_tools(state);
    let mcp_decls = state.mcp_client.build_gemini_tool_declarations().await;

    if mcp_decls.is_empty() {
        return native;
    }

    // MCP tools go FIRST — position advantage for model tool selection
    let mut result = native.clone();
    if let Some(arr) = result
        .get_mut(0)
        .and_then(|v| v.get_mut("function_declarations"))
        .and_then(|v| v.as_array_mut())
    {
        let native_tools: Vec<serde_json::Value> = std::mem::take(arr);
        arr.extend(mcp_decls);
        arr.extend(native_tools);
    }
    result
}
