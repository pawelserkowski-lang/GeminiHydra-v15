// backend/src/files/validator.rs
use crate::files::FileError;
use std::path::{Path, PathBuf};

/// Backup / swap file extensions that should never be read or written.
const BLOCKED_EXTENSIONS: &[&str] = &[".bak", ".old", ".orig", ".swp", ".swo"];

/// Text file extensions we allow reading.
const TEXT_EXTENSIONS: &[&str] = &[
    // Code
    "rs",
    "ts",
    "tsx",
    "js",
    "jsx",
    "py",
    "go",
    "java",
    "kt",
    "c",
    "cpp",
    "h",
    "hpp",
    "cs",
    "rb",
    "php",
    "swift",
    "scala",
    "zig",
    "lua",
    "r",
    "sql",
    "sh",
    "bash",
    "zsh",
    "ps1",
    "bat",
    "cmd",
    // Code — additional module formats
    "mjs",
    "cjs",
    "mts",
    "cts",
    // Config / Data
    "json",
    "yaml",
    "yml",
    "toml",
    "xml",
    "csv",
    "env",
    "ini",
    "cfg",
    "properties",
    "lock",
    // Schema / IaC
    "graphql",
    "gql",
    "proto",
    "prisma",
    "gradle",
    "tf",
    "hcl",
    "dockerfile",
    "makefile",
    "cmake",
    // Web
    "html",
    "htm",
    "css",
    "scss",
    "sass",
    "less",
    "svg",
    // Web — frameworks
    "svelte",
    "vue",
    "astro",
    // Templating
    "njk",
    "ejs",
    "hbs",
    "pug",
    // Docs
    "md",
    "txt",
    "rst",
    "log",
    "gitignore",
    "dockerignore",
    "editorconfig",
];

/// Extension-less filenames recognized as text files.
const TEXT_FILENAMES: &[&str] = &[
    "Dockerfile",
    "Makefile",
    "Makefile.am",
    "Rakefile",
    "Gemfile",
    "Procfile",
    "Vagrantfile",
    "Justfile",
    "Taskfile",
    ".gitignore",
    ".dockerignore",
    ".editorconfig",
    ".eslintrc",
    ".prettierrc",
    ".babelrc",
    ".npmrc",
    ".nvmrc",
    ".env.example",
    ".env.local",
    ".env.production",
    ".env.development",
];

pub fn is_text_extension(ext: &str) -> bool {
    TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

pub fn is_text_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str())
        && is_text_extension(ext)
    {
        return true;
    }
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        return TEXT_FILENAMES.contains(&name);
    }
    false
}

pub fn validate_and_canonicalize(
    raw_path: &str,
    blocked_prefixes: &[&str],
) -> Result<PathBuf, FileError> {
    if raw_path.contains('\0') {
        return Err(FileError {
            path: raw_path.to_string(),
            reason: "Access denied: path contains null byte".to_string(),
        });
    }

    if raw_path.starts_with("\\\\") || raw_path.starts_with("//") {
        return Err(FileError {
            path: raw_path.to_string(),
            reason: "Access denied: UNC/network paths are not allowed".to_string(),
        });
    }

    {
        let check_path = raw_path;
        let after_drive = if check_path.len() >= 2
            && check_path.as_bytes()[0].is_ascii_alphabetic()
            && check_path.as_bytes()[1] == b':'
        {
            &check_path[2..]
        } else {
            check_path
        };
        if after_drive.contains(':') {
            return Err(FileError {
                path: raw_path.to_string(),
                reason: "Access denied: Windows alternate data streams (ADS) are not allowed"
                    .to_string(),
            });
        }
    }

    let path_lower = raw_path.to_lowercase();
    for ext in BLOCKED_EXTENSIONS {
        if path_lower.ends_with(ext) {
            return Err(FileError {
                path: raw_path.to_string(),
                reason: format!(
                    "Access denied: backup/swap file extension '{}' is not allowed",
                    ext
                ),
            });
        }
    }
    if raw_path.ends_with('~') {
        return Err(FileError {
            path: raw_path.to_string(),
            reason: "Access denied: tilde backup files are not allowed".to_string(),
        });
    }

    let p = Path::new(raw_path);
    let canonical = if p.exists() {
        std::fs::canonicalize(p).map_err(|e| FileError {
            path: raw_path.to_string(),
            reason: format!("Cannot resolve path: {}", e),
        })?
    } else {
        let parent = p.parent().ok_or_else(|| FileError {
            path: raw_path.to_string(),
            reason: "Invalid path: no parent directory".to_string(),
        })?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|e| FileError {
            path: raw_path.to_string(),
            reason: format!("Cannot resolve parent directory: {}", e),
        })?;
        let file_name = p.file_name().ok_or_else(|| FileError {
            path: raw_path.to_string(),
            reason: "Invalid path: no filename".to_string(),
        })?;
        canonical_parent.join(file_name)
    };

    let canonical_str = canonical.to_string_lossy();
    let canonical_win = canonical_str.replace('/', "\\");
    let canonical_lower = canonical_str.to_lowercase();
    let canonical_win_lower = canonical_win.to_lowercase();

    for prefix in blocked_prefixes {
        let prefix_lower = prefix.to_lowercase();
        if canonical_lower.starts_with(&prefix_lower)
            || canonical_win_lower.starts_with(&prefix_lower)
        {
            return Err(FileError {
                path: raw_path.to_string(),
                reason: format!(
                    "Access denied: path '{}' resolves to blocked location '{}'",
                    raw_path,
                    canonical.display()
                ),
            });
        }
    }

    Ok(canonical)
}
