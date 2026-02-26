// backend/src/analysis.rs
//! Static Code Analysis module using Tree-sitter.
//!
//! Provides AST-based structure extraction to give agents a high-level view
//! of code without reading entire file contents.

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

pub fn analyze_file(path: &str, content: &str) -> Option<FileStructure> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let mut parser = Parser::new();
    let language = match extension {
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        "ts" | "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "js" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
        "py" => tree_sitter_python::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        _ => return None,
    };

    parser.set_language(&language).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();

    let query_variants = match extension {
        "rs" => rust_queries(),
        "ts" | "tsx" | "js" | "jsx" => ts_queries(),
        "py" => py_queries(),
        "go" => go_queries(),
        _ => return None,
    };

    // Try each query variant until one succeeds
    let query = query_variants.iter()
        .find_map(|qs| Query::new(&language, qs).ok())?;

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
