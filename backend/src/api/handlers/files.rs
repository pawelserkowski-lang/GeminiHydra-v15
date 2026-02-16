use axum::Json;
use serde_json::{json, Value};
use crate::models::{FileReadRequest, FileReadResponse, FileListRequest, FileListResponse, FileEntryResponse};
use crate::files;

pub async fn read_file(Json(body): Json<FileReadRequest>) -> Json<Value> {
    match files::read_file_raw(&body.path).await {
        Ok(f) => Json(json!(FileReadResponse { path: f.path, content: f.content, size_bytes: f.size_bytes, truncated: f.truncated, extension: f.extension })),
        Err(e) => Json(json!({ "error": e.reason, "path": e.path })),
    }
}

pub async fn list_files(Json(body): Json<FileListRequest>) -> Json<Value> {
    match files::list_directory(&body.path, body.show_hidden).await {
        Ok(e) => {
            let res: Vec<_> = e.into_iter().map(|i| FileEntryResponse { name: i.name, path: i.path, is_dir: i.is_dir, size_bytes: i.size_bytes, extension: i.extension }).collect();
            Json(json!(FileListResponse { path: body.path, count: res.len(), entries: res }))
        }
        Err(e) => Json(json!({ "error": e.reason, "path": e.path })),
    }
}
