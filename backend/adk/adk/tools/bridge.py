"""HTTP bridge tools — proxy calls to Rust backend's tool execution endpoint.

Each function is a standard Python function with type hints and docstrings.
ADK auto-generates tool schemas from these signatures.
The Rust backend at POST /api/internal/tool handles actual execution.
"""

import httpx
from adk.config import RUST_BACKEND_URL, AUTH_SECRET

_headers = {"Content-Type": "application/json"}
if AUTH_SECRET:
    _headers["Authorization"] = f"Bearer {AUTH_SECRET}"


def _call_tool(name: str, args: dict) -> dict:
    """Call Rust backend tool endpoint synchronously."""
    with httpx.Client(base_url=RUST_BACKEND_URL, timeout=60.0, headers=_headers) as client:
        resp = client.post("/api/internal/tool", json={"name": name, "args": args})
        resp.raise_for_status()
        return resp.json()


def list_directory(path: str, show_hidden: bool = False) -> dict:
    """List files and subdirectories in a local directory on the Windows machine.

    Args:
        path: Absolute path to directory (e.g. "C:/Users/BIURODOM/Desktop/GeminiHydra-v15").
        show_hidden: Whether to include hidden files starting with dot.

    Returns:
        dict with status and directory listing including file sizes and line counts.
    """
    return _call_tool("list_directory", {"path": path, "show_hidden": show_hidden})


def read_file(path: str) -> dict:
    """Read the full contents of a file from the local filesystem.

    Args:
        path: Absolute path to the file (e.g. "C:/Users/BIURODOM/Desktop/GeminiHydra-v15/backend/src/main.rs").

    Returns:
        dict with status and file contents as text.
    """
    return _call_tool("read_file", {"path": path})


def read_file_section(path: str, start_line: int, end_line: int) -> dict:
    """Read a specific range of lines from a file (1-indexed).

    Args:
        path: Absolute path to the file.
        start_line: First line to read (1-indexed, inclusive).
        end_line: Last line to read (1-indexed, inclusive). Max 500 lines per call.

    Returns:
        dict with status and the requested line range.
    """
    return _call_tool("read_file_section", {
        "path": path, "start_line": start_line, "end_line": end_line
    })


def search_files(path: str, pattern: str, file_extensions: str = "",
                 offset: int = 0, limit: int = 80, multiline: bool = False) -> dict:
    """Search for text or regex patterns across all files in a directory recursively.

    Args:
        path: Root directory to search in.
        pattern: Regex pattern to search for.
        file_extensions: Comma-separated file extensions to filter (e.g. "rs,ts,py"). Empty = all text files.
        offset: Number of results to skip (pagination).
        limit: Maximum number of results to return (default 80, max 150).
        multiline: Whether to enable multiline regex matching.

    Returns:
        dict with status and matching file locations with context.
    """
    args: dict = {"path": path, "pattern": pattern, "offset": offset,
                  "limit": limit, "multiline": multiline}
    if file_extensions:
        args["file_extensions"] = file_extensions
    return _call_tool("search_files", args)


def find_file(path: str, pattern: str) -> dict:
    """Find files by name pattern (glob-style) in a directory tree.

    Args:
        path: Root directory to search in.
        pattern: Glob pattern (e.g. "*.rs", "**/*.tsx", "Cargo.*").

    Returns:
        dict with status and list of matching file paths.
    """
    return _call_tool("find_file", {"path": path, "pattern": pattern})


def get_code_structure(path: str) -> dict:
    """Analyze the code structure of a source file using AST parsing.

    Extracts functions, classes, structs, enums, traits, interfaces, and their signatures.
    Uses tree-sitter AST parsing with regex fallback for modern syntax.
    Supports: Rust, TypeScript/TSX, JavaScript/JSX, Python, Go.

    Args:
        path: Absolute path to a source code file.

    Returns:
        dict with status and extracted code symbols with line numbers.
    """
    return _call_tool("get_code_structure", {"path": path})


def write_file(path: str, content: str) -> dict:
    """Create or overwrite a file on the local filesystem.

    Args:
        path: Absolute path for the file to write.
        content: The full text content to write to the file.

    Returns:
        dict with status confirming the write operation.
    """
    return _call_tool("write_file", {"path": path, "content": content})


def edit_file(path: str, old_text: str, new_text: str) -> dict:
    """Make a targeted text replacement in an existing file.

    Finds the exact old_text in the file and replaces it with new_text.
    Safer than write_file for small changes — preserves the rest of the file.

    Args:
        path: Absolute path to the file to edit.
        old_text: The exact text to find and replace (must match uniquely).
        new_text: The replacement text.

    Returns:
        dict with status confirming the edit.
    """
    return _call_tool("edit_file", {"path": path, "old_text": old_text, "new_text": new_text})


def diff_files(path_a: str, path_b: str) -> dict:
    """Compare two files and show line-by-line differences.

    Args:
        path_a: Absolute path to the first file.
        path_b: Absolute path to the second file.

    Returns:
        dict with status and unified diff output.
    """
    return _call_tool("diff_files", {"path_a": path_a, "path_b": path_b})


def execute_command(command: str, working_directory: str = "") -> dict:
    """Execute a shell command on the local Windows machine using cmd.exe.

    LAST RESORT ONLY — use dedicated tools (list_directory, read_file, search_files,
    get_code_structure) instead whenever possible. NEVER use this for file reading,
    listing, searching, or code analysis.

    Args:
        command: The shell command to execute (runs in cmd.exe, NOT PowerShell).
        working_directory: Optional working directory for the command.

    Returns:
        dict with status, stdout, and stderr. 30 second timeout.
    """
    args: dict = {"command": command}
    if working_directory:
        args["working_directory"] = working_directory
    return _call_tool("execute_command", args)


# All bridge tools as a list for agent registration
ALL_TOOLS = [
    list_directory,
    read_file,
    read_file_section,
    search_files,
    find_file,
    get_code_structure,
    write_file,
    edit_file,
    diff_files,
    execute_command,
]
