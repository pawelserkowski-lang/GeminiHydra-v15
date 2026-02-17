// backend/src/analysis.rs
//! Static Code Analysis module using Tree-sitter.
//!
//! Provides AST-based structure extraction to give agents a high-level view
//! of code without reading entire file contents.

use serde::Serialize;
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

pub fn analyze_file(path: &str, content: &str) -> Option<FileStructure> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let mut parser = Parser::new();
    let language = match extension {
        "rs" => tree_sitter_rust::language(),
        "ts" | "tsx" => tree_sitter_typescript::language_tsx(),
        "js" | "jsx" => tree_sitter_javascript::language(),
        "py" => tree_sitter_python::language(),
        "go" => tree_sitter_go::language(),
        _ => return None,
    };

    parser.set_language(language).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();

    let query_str = match extension {
        "rs" => r#"
            (function_item name: (identifier) @name) @func
            (struct_item name: (type_identifier) @name) @struct
            (impl_item type: (type_identifier) @name) @impl
            (trait_item name: (type_identifier) @name) @trait
            (mod_item name: (identifier) @name) @mod
        "#,
        "ts" | "tsx" | "js" | "jsx" => r#"
            (function_declaration name: (identifier) @name) @func
            (class_declaration name: (type_identifier) @name) @class
            (interface_declaration name: (type_identifier) @name) @interface
            (type_alias_declaration name: (type_identifier) @name) @type
            (method_definition name: (property_identifier) @name) @method
            (export_statement declaration: (_) @decl)
        "#,
        "py" => r#"
            (function_definition name: (identifier) @name) @func
            (class_definition name: (identifier) @name) @class
        "#,
        "go" => r#"
            (function_declaration name: (identifier) @name) @func
            (type_declaration (type_spec name: (type_identifier) @name)) @type
            (method_declaration name: (field_identifier) @name) @method
        "#,
        _ => return None,
    };

    let query = Query::new(language, query_str).ok()?;
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let lines: Vec<&str> = content.lines().collect();

    for m in cursor.matches(&query, root, content.as_bytes()) {
        let node = m.captures[0].node;
        let kind = node.kind().to_string();
        
        // Try to extract name from @name capture if present
        let name = m.captures.iter().find(|_c| m.pattern_index < query.pattern_count())
            .and_then(|_| {
                // In tree-sitter queries, captures are indexed. 
                // We simplify: just grab the text of the node that looks like a name.
                // Or use the node text itself if it's small.
                
                // Let's find the capture named "name"
                let idx = query.capture_index_for_name("name")?;
                m.captures.iter().find(|c| c.index == idx).map(|c| {
                    let r = c.node.byte_range();
                    if r.end <= content.len() {
                        content[r].to_string()
                    } else {
                        "???".to_string()
                    }
                })
            })
            .unwrap_or_else(|| "anonymous".to_string());

        // Get signature (first line of definition)
        let start_line = node.start_position().row;
        let signature = if start_line < lines.len() {
            lines[start_line].trim().to_string()
        } else {
            kind.clone()
        };

        // Dedup: ignore generic matches that don't look like symbols
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
