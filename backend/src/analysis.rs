// backend/src/analysis.rs
//! Static Code Analysis module using Tree-sitter with regex fallback.
//!
//! Provides AST-based structure extraction to give agents a high-level view
//! of code without reading entire file contents.
//!
//! Two-stage approach:
//! 1. Tree-sitter AST parsing (most accurate, handles complex syntax)
//! 2. Regex-based fallback (handles newer syntax tree-sitter may not support,
//!    e.g. Rust let-chains, or when tree-sitter grammar version mismatches)

use regex::Regex;
use serde::Serialize;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug, Serialize)]
pub struct CodeSymbol {
    pub kind: String, // "function", "class", "impl", "interface", "struct"
    pub name: String,
    pub signature: String,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct FileStructure {
    pub path: String,
    pub symbols: Vec<CodeSymbol>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyze a source file and extract its code structure (functions, classes, etc.).
///
/// Uses tree-sitter AST analysis first; falls back to regex-based extraction
/// when tree-sitter fails (e.g., for files using newer language syntax).
pub fn analyze_file(path: &str, content: &str) -> Option<FileStructure> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Check if we support this extension at all
    if !matches!(extension, "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go") {
        return None;
    }

    // Try tree-sitter AST analysis first (most accurate)
    if let Some(result) = analyze_treesitter(path, content, extension) {
        if !result.symbols.is_empty() {
            return Some(result);
        }
    }

    // Fallback: regex-based analysis (handles newer syntax tree-sitter may not support)
    tracing::debug!("Tree-sitter returned no symbols for '{}', using regex fallback", path);
    let result = analyze_regex(path, content, extension);
    if result.symbols.is_empty() { None } else { Some(result) }
}

// ---------------------------------------------------------------------------
// Tree-sitter Analysis
// ---------------------------------------------------------------------------

/// Query definitions for each supported language.
/// Each language may provide multiple query variants: if the first fails
/// (due to grammar version differences), we fall back to simpler patterns.
fn rust_queries() -> &'static [&'static str] {
    &[
        // Primary: detailed Rust query
        r#"
            (function_item name: (identifier) @name) @func
            (struct_item name: (type_identifier) @name) @struct
            (impl_item type: (type_identifier) @name) @impl
            (trait_item name: (type_identifier) @name) @trait
            (mod_item name: (identifier) @name) @mod
            (enum_item name: (type_identifier) @name) @enum
        "#,
        // Fallback: simpler patterns if grammar names differ
        r#"
            (function_item) @func
            (struct_item) @struct
            (impl_item) @impl
            (trait_item) @trait
        "#,
    ]
}

fn ts_queries() -> &'static [&'static str] {
    &[
        r#"
            (function_declaration name: (identifier) @name) @func
            (class_declaration name: (type_identifier) @name) @class
            (interface_declaration name: (type_identifier) @name) @interface
            (type_alias_declaration name: (type_identifier) @name) @type
            (method_definition name: (property_identifier) @name) @method
            (export_statement declaration: (_) @decl)
        "#,
    ]
}

fn py_queries() -> &'static [&'static str] {
    &[
        r#"
            (function_definition name: (identifier) @name) @func
            (class_definition name: (identifier) @name) @class
        "#,
    ]
}

fn go_queries() -> &'static [&'static str] {
    &[
        r#"
            (function_declaration name: (identifier) @name) @func
            (type_declaration (type_spec name: (type_identifier) @name)) @type
            (method_declaration name: (field_identifier) @name) @method
        "#,
    ]
}

fn analyze_treesitter(path: &str, content: &str, extension: &str) -> Option<FileStructure> {
    let mut parser = Parser::new();
    let language = match extension {
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        "ts" | "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "js" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
        "py" => tree_sitter_python::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        _ => return None,
    };

    if parser.set_language(&language).is_err() {
        tracing::warn!("Tree-sitter: failed to set language for extension '{}'", extension);
        return None;
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => {
            tracing::warn!("Tree-sitter: failed to parse '{}'", path);
            return None;
        }
    };
    let root = tree.root_node();

    let query_variants = match extension {
        "rs" => rust_queries(),
        "ts" | "tsx" | "js" | "jsx" => ts_queries(),
        "py" => py_queries(),
        "go" => go_queries(),
        _ => return None,
    };

    // Try each query variant until one succeeds
    let query = match query_variants.iter().find_map(|qs| Query::new(&language, qs).ok()) {
        Some(q) => q,
        None => {
            tracing::warn!("Tree-sitter: all query variants failed for '{}'", path);
            return None;
        }
    };

    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let name_idx = query.capture_index_for_name("name");

    let mut matches = cursor.matches(&query, root, content.as_bytes());
    while let Some(m) = matches.next() {
        let node = m.captures[0].node;
        let kind = node.kind().to_string();

        // Extract @name capture if present, otherwise try to infer from node's first named child
        let name = name_idx
            .and_then(|idx| {
                m.captures.iter().find(|c| c.index == idx).map(|c| {
                    let r = c.node.byte_range();
                    if r.end <= content.len() { content[r].to_string() } else { "???".to_string() }
                })
            })
            .or_else(|| {
                // Fallback: look for first named child that's an identifier or type_identifier
                let mut child_cursor = node.walk();
                for child in node.named_children(&mut child_cursor) {
                    let ck = child.kind();
                    if ck == "identifier" || ck == "type_identifier" {
                        let r = child.byte_range();
                        if r.end <= content.len() {
                            return Some(content[r].to_string());
                        }
                    }
                }
                None
            })
            .unwrap_or_else(|| "anonymous".to_string());

        let start_line = node.start_position().row;
        let signature = if start_line < lines.len() {
            lines[start_line].trim().to_string()
        } else {
            kind.clone()
        };

        if name == "anonymous" && !kind.contains("export") {
            continue;
        }

        symbols.push(CodeSymbol {
            kind,
            name,
            signature,
            line: start_line + 1,
        });
    }

    Some(FileStructure {
        path: path.to_string(),
        symbols,
    })
}

// ---------------------------------------------------------------------------
// Regex-based Fallback
// ---------------------------------------------------------------------------

/// Regex-based code structure extraction. Less accurate than tree-sitter
/// but handles files with newer syntax that tree-sitter grammars may not support.
fn analyze_regex(path: &str, content: &str, extension: &str) -> FileStructure {
    let patterns: &[(&str, &str)] = match extension {
        "rs" => &[
            (r"(?m)^\s*(?:pub(?:\(crate\))?\s+)?(?:async\s+)?fn\s+(\w+)", "function"),
            (r"(?m)^\s*(?:pub(?:\(crate\))?\s+)?struct\s+(\w+)", "struct"),
            (r"(?m)^\s*(?:pub(?:\(crate\))?\s+)?enum\s+(\w+)", "enum"),
            (r"(?m)^\s*(?:pub(?:\(crate\))?\s+)?trait\s+(\w+)", "trait"),
            (r"(?m)^impl(?:<[^>]*>)?\s+(?:(\w+)\s+for\s+)?(\w+)", "impl"),
            (r"(?m)^\s*(?:pub(?:\(crate\))?\s+)?mod\s+(\w+)", "mod"),
        ],
        "ts" | "tsx" | "js" | "jsx" => &[
            (r"(?m)^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s+(\w+)", "function"),
            (r"(?m)^\s*(?:export\s+)?(?:default\s+)?class\s+(\w+)", "class"),
            (r"(?m)^\s*(?:export\s+)?interface\s+(\w+)", "interface"),
            (r"(?m)^\s*(?:export\s+)?type\s+(\w+)", "type"),
            (r"(?m)^\s*(?:export\s+)?(?:const|let)\s+(\w+)\s*=\s*(?:async\s+)?\(", "arrow_fn"),
        ],
        "py" => &[
            (r"(?m)^\s*(?:async\s+)?def\s+(\w+)", "function"),
            (r"(?m)^\s*class\s+(\w+)", "class"),
        ],
        "go" => &[
            (r"(?m)^func\s+(?:\(\w+\s+\*?\w+\)\s+)?(\w+)", "function"),
            (r"(?m)^type\s+(\w+)\s+struct", "struct"),
            (r"(?m)^type\s+(\w+)\s+interface", "interface"),
        ],
        _ => &[],
    };

    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for &(pattern, kind) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            for cap in re.captures_iter(content) {
                // Use the last non-None capture group (handles impl X for Y case)
                let name = (1..cap.len())
                    .rev()
                    .find_map(|i| cap.get(i).map(|m| m.as_str().to_string()))
                    .unwrap_or_else(|| "anonymous".to_string());

                if name == "anonymous" { continue; }

                let byte_offset = cap.get(0).map(|m| m.start()).unwrap_or(0);
                let line_num = content[..byte_offset].matches('\n').count();
                let signature = if line_num < lines.len() {
                    lines[line_num].trim().to_string()
                } else {
                    kind.to_string()
                };

                symbols.push(CodeSymbol {
                    kind: kind.to_string(),
                    name,
                    signature,
                    line: line_num + 1,
                });
            }
        }
    }

    // Sort by line number and deduplicate
    symbols.sort_by_key(|s| s.line);
    symbols.dedup_by(|a, b| a.line == b.line && a.name == b.name);

    FileStructure {
        path: path.to_string(),
        symbols,
    }
}
