// tools/zip_tools.rs
// Jaskier Shared Pattern -- zip_tools
//! ZIP archive tools for agent function calling.

use std::io::Read;
use std::path::Path;

/// Maximum archive size (100 MB).
const MAX_ARCHIVE_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum single file extraction size (10 MB).
const MAX_EXTRACT_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum output text length.
const MAX_OUTPUT_CHARS: usize = 6000;

/// List contents of a ZIP archive.
pub async fn tool_list_zip(path: &str) -> Result<String, String> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|e| format!("Cannot read metadata: {}", e))?;
    if metadata.len() > MAX_ARCHIVE_SIZE {
        return Err(format!(
            "Archive too large: {} bytes (max {} MB)",
            metadata.len(),
            MAX_ARCHIVE_SIZE / (1024 * 1024)
        ));
    }

    let path_owned = path.to_string();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path_owned)
            .map_err(|e| format!("Cannot open file: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Not a valid ZIP archive: {}", e))?;

        let mut output = format!(
            "### ZIP: {} ({} entries)\n\n{:<60} {:>12} {:>12}\n{}\n",
            Path::new(&path_owned)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("archive.zip"),
            archive.len(),
            "Name",
            "Size",
            "Compressed",
            "-".repeat(86)
        );

        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| format!("Error reading entry: {}", e))?;
            let name = entry.name().to_string();
            let size = entry.size();
            let compressed = entry.compressed_size();
            let is_dir = entry.is_dir();

            let line = if is_dir {
                format!("{:<60} {:>12} {:>12}\n", format!("{}/", name), "-", "-")
            } else {
                format!(
                    "{:<60} {:>12} {:>12}\n",
                    name,
                    format_size(size),
                    format_size(compressed)
                )
            };
            output.push_str(&line);

            if output.len() > MAX_OUTPUT_CHARS {
                output.push_str("\n[... truncated ...]");
                break;
            }
        }

        Ok(output)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Extract a single file from a ZIP archive.
pub async fn tool_extract_zip_file(path: &str, file_path: &str) -> Result<String, String> {
    let archive_path = Path::new(path);

    if !archive_path.exists() {
        return Err(format!("Archive not found: {}", path));
    }

    // Zip slip prevention
    if !is_safe_zip_entry(file_path) {
        return Err(format!(
            "Unsafe path in ZIP (path traversal blocked): {}",
            file_path
        ));
    }

    let path_owned = path.to_string();
    let file_path_owned = file_path.to_string();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path_owned)
            .map_err(|e| format!("Cannot open archive: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Not a valid ZIP archive: {}", e))?;

        let mut entry = archive
            .by_name(&file_path_owned)
            .map_err(|_| format!("File not found in archive: {}", file_path_owned))?;

        if entry.size() > MAX_EXTRACT_SIZE {
            return Err(format!(
                "File too large to extract: {} bytes (max {} MB)",
                entry.size(),
                MAX_EXTRACT_SIZE / (1024 * 1024)
            ));
        }

        let mut buffer = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;

        // Check if binary
        let is_binary = buffer.iter().take(8192).any(|&b| b == 0);

        let filename = Path::new(&file_path_owned)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        if is_binary {
            let hex_preview: String = buffer
                .iter()
                .take(256)
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .chunks(32)
                .map(|chunk| chunk.join(" "))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!(
                "### ZIP Entry: {} (binary, {} bytes)\n\nHex preview (first 256 bytes):\n```\n{}\n```",
                filename,
                buffer.len(),
                hex_preview
            ))
        } else {
            let text = String::from_utf8_lossy(&buffer);
            let mut output = format!("### ZIP Entry: {} ({} bytes)\n\n", filename, buffer.len());
            if text.len() > MAX_OUTPUT_CHARS - output.len() {
                let available = MAX_OUTPUT_CHARS.saturating_sub(output.len() + 40);
                let truncated: String = text.chars().take(available).collect();
                output.push_str(&truncated);
                output.push_str("\n\n[... truncated ...]");
            } else {
                output.push_str(&text);
            }
            Ok(output)
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Zip slip prevention -- reject unsafe entry names.
fn is_safe_zip_entry(name: &str) -> bool {
    !name.contains("..")
        && !name.starts_with('/')
        && !name.starts_with('\\')
        && !name.contains(":\\")
        && !name.contains(":/")
}

/// Format byte size for display.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
