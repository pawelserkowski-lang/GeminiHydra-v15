// Jaskier Shared Pattern -- ocr
// GeminiHydra v15 — OCR via Gemini Vision API
//
// Endpoints:
//   POST /api/ocr        — synchronous OCR (single image or PDF)
//   POST /api/ocr/stream — SSE streaming OCR with progress events

use std::convert::Infallible;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::oauth;
use crate::state::AppState;

// ── Constants ────────────────────────────────────────────────────────────────

const OCR_PROMPT: &str = "\
Extract ALL text from this document/image exactly as written.\n\
\n\
FORMATTING RULES:\n\
- Preserve line breaks, paragraph structure, and reading order (left-to-right, top-to-bottom)\n\
- Use # markdown headers for section titles and document headings\n\
- Format key:value pairs as **Key:** Value\n\
- CRITICAL: Format ALL tables using markdown pipe table syntax, like this:\n\
  | Column A | Column B | Column C |\n\
  |----------|----------|----------|\n\
  | data 1   | data 2   | data 3   |\n\
- Preserve special characters, numbers, currencies, and diacritics exactly\n\
- If the text is handwritten, transcribe it as accurately as possible\n\
- For multi-page documents, insert ---PAGE 2---, ---PAGE 3--- etc. markers between pages (use the actual page number)\n\
\n\
Return ONLY the extracted text with markdown formatting, no descriptions or commentary.";

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const OCR_MODEL: &str = "gemini-3-flash-preview";
const MAX_INPUT_SIZE: usize = 30_000_000; // ~22 MB decoded

// ── Request / Response models ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OcrRequest {
    /// Base64-encoded image or PDF data.
    pub data_base64: String,
    /// MIME type: image/png, image/jpeg, image/webp, application/pdf
    pub mime_type: String,
    /// Optional custom prompt (overrides default OCR_PROMPT).
    pub prompt: Option<String>,
    /// For PDFs: optional page range (informational).
    pub page_range: Option<String>,
    /// Language hint (e.g. "pl", "en", "de") — improves accuracy for diacritics and domain terms.
    pub language: Option<String>,
    /// OCR preset: "invoice", "document", "handwriting", "table", "receipt".
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrResponse {
    pub text: String,
    pub pages: Vec<OcrPage>,
    pub total_pages: usize,
    pub processing_time_ms: u64,
    pub provider: String,
    /// Approximate confidence score (0.0–1.0) derived from model logprobs, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrPage {
    pub page_number: usize,
    pub text: String,
}

// ── Prompt builder ──────────────────────────────────────────────────────────

/// Build the effective OCR prompt from base prompt + optional language/preset.
fn build_ocr_prompt(base: &str, language: Option<&str>, preset: Option<&str>) -> String {
    let mut prompt = base.to_string();

    if let Some(lang) = language {
        prompt.push_str(&format!(
            "\n\nThe document is in {lang}. Pay special attention to language-specific \
             characters, diacritics, and terminology."
        ));
    }

    if let Some(preset) = preset {
        let extra = match preset {
            "invoice" => "\n\nThis is an INVOICE/RECEIPT. You MUST extract:\n\
                - Seller and buyer details (name, address, NIP/VAT ID)\n\
                - Invoice number and dates\n\
                - Line items table with columns: Lp./Name/Qty/Unit/Net price/Net value/VAT rate/VAT amount/Gross value\n\
                - Summary totals (net, VAT, gross)\n\
                - Payment details (bank account, due date)",
            "receipt" => "\n\nThis is a RECEIPT. Extract all items with prices in a table. \
                Include store name, date, total, and payment method.",
            "table" => "\n\nThis document contains IMPORTANT TABLES. Extract EVERY table with \
                precise alignment of columns and rows using markdown pipe syntax. Do not skip any cells.",
            "handwriting" => "\n\nThis contains HANDWRITTEN text. Take extra care to:\n\
                - Distinguish similar characters (l/1, O/0, n/u)\n\
                - Preserve crossed-out text as ~~strikethrough~~\n\
                - Mark illegible words as [illegible]",
            "document" => "\n\nThis is an official DOCUMENT. Preserve:\n\
                - All form fields and their values as key:value pairs\n\
                - Official stamps and signatures as [stamp] / [signature]\n\
                - Reference numbers, dates, and legal identifiers exactly",
            _ => "",
        };
        prompt.push_str(extra);
    }

    prompt
}

// ── Synchronous OCR endpoint ─────────────────────────────────────────────────

pub async fn ocr(
    State(state): State<AppState>,
    Json(body): Json<OcrRequest>,
) -> Result<Json<OcrResponse>, impl IntoResponse> {
    if body.data_base64.len() > MAX_INPUT_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error": "Input exceeds maximum size (22 MB)"})),
        ));
    }

    let started = Instant::now();

    let base_prompt = body.prompt.as_deref().unwrap_or(OCR_PROMPT);
    let effective_prompt = build_ocr_prompt(
        base_prompt,
        body.language.as_deref(),
        body.preset.as_deref(),
    );

    let (text, confidence) = ocr_with_gemini(
        &state,
        &body.data_base64,
        &body.mime_type,
        &effective_prompt,
    )
    .await
    .map_err(|e| {
        tracing::error!("OCR failed: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        )
    })?;

    let pages = split_into_pages(&text);
    let total_pages = pages.len().max(1);

    Ok(Json(OcrResponse {
        text: text.clone(),
        pages,
        total_pages,
        processing_time_ms: started.elapsed().as_millis() as u64,
        provider: "gemini".to_string(),
        confidence,
    }))
}

// ── SSE streaming OCR endpoint ───────────────────────────────────────────────

pub async fn ocr_stream(
    State(state): State<AppState>,
    Json(body): Json<OcrRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, impl IntoResponse> {
    if body.data_base64.len() > MAX_INPUT_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"error": "Input exceeds maximum size (22 MB)"})),
        ));
    }

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let started = Instant::now();

        // ocr/start
        send_progress(&tx, "start", None).await;

        let base_prompt = body.prompt.as_deref().unwrap_or(OCR_PROMPT);
        let effective_prompt = build_ocr_prompt(
            base_prompt,
            body.language.as_deref(),
            body.preset.as_deref(),
        );
        let result = ocr_with_gemini(&state, &body.data_base64, &body.mime_type, &effective_prompt).await;

        match result {
            Ok((text, confidence)) => {
                let pages = split_into_pages(&text);
                let total = pages.len().max(1);

                // ocr/progress for each page
                for (i, page) in pages.iter().enumerate() {
                    let preview = if page.text.len() > 200 {
                        let end = page.text.char_indices()
                            .take_while(|(i, _)| *i < 200)
                            .last()
                            .map(|(i, c)| i + c.len_utf8())
                            .unwrap_or(200);
                        format!("{}...", &page.text[..end])
                    } else {
                        page.text.clone()
                    };
                    send_progress(
                        &tx,
                        "progress",
                        Some(&json!({
                            "pages_done": i + 1,
                            "pages_total": total,
                            "progress": (i + 1) as f64 / total as f64,
                            "current_page_preview": preview,
                            "elapsed_seconds": started.elapsed().as_secs(),
                        })),
                    )
                    .await;
                }

                // ocr/done
                send_progress(&tx, "done", None).await;

                // complete event with full response
                let response = OcrResponse {
                    text,
                    pages,
                    total_pages: total,
                    processing_time_ms: started.elapsed().as_millis() as u64,
                    provider: "gemini".to_string(),
                    confidence,
                };
                let complete = Event::default()
                    .event("complete")
                    .data(serde_json::to_string(&response).unwrap_or_default());
                let _ = tx.send(Ok(complete)).await;
            }
            Err(e) => {
                tracing::error!("OCR stream failed: {e}");
                let err_event = Event::default()
                    .event("error")
                    .data(json!({"error": e}).to_string());
                let _ = tx.send(Ok(err_event)).await;
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// ── Gemini Vision OCR core ───────────────────────────────────────────────────

async fn ocr_with_gemini(
    state: &AppState,
    data_b64: &str,
    mime_type: &str,
    prompt: &str,
) -> Result<(String, Option<f64>), String> {
    let (credential, is_oauth) = oauth::get_google_credential(state)
        .await
        .ok_or_else(|| "No Google API credential configured".to_string())?;

    let url = format!("{GEMINI_API_BASE}/{OCR_MODEL}:generateContent");

    let request_body = json!({
        "contents": [{
            "parts": [
                {
                    "inlineData": {
                        "mimeType": mime_type,
                        "data": data_b64
                    }
                },
                {
                    "text": prompt
                }
            ]
        }],
        "generationConfig": {
            "temperature": 1.0,
            "maxOutputTokens": 16384
        }
    });

    let builder = state.client.post(&url).json(&request_body);
    let builder = oauth::apply_google_auth(builder, &credential, is_oauth);

    let response = builder
        .send()
        .await
        .map_err(|e| format!("Gemini API request failed: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {e}"))?;

    if !status.is_success() {
        let msg = body["error"]["message"]
            .as_str()
            .unwrap_or("Unknown Gemini API error");
        return Err(format!("Gemini API error ({status}): {msg}"));
    }

    // Extract text from response
    let text = body["candidates"][0]["content"]["parts"]
        .as_array()
        .and_then(|parts| {
            parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .next()
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    if text.is_empty() {
        return Err("Gemini returned empty OCR result".to_string());
    }

    // Extract confidence from avgLogprobs (if available)
    let confidence = body["candidates"][0]["avgLogprobs"]
        .as_f64()
        .map(|logprob| logprob.exp().clamp(0.0, 1.0));

    Ok((text, confidence))
}

// ── Public helper for agent tool fallback ─────────────────────────────────────

/// OCR a PDF document via Gemini Vision API. Used by `read_pdf` tool as fallback
/// when `pdf-extract` returns empty text (scanned/image-based PDFs).
pub async fn ocr_pdf_text(
    state: &AppState,
    data_b64: &str,
    _page_range: Option<&str>,
) -> Result<String, String> {
    ocr_with_gemini(state, data_b64, "application/pdf", OCR_PROMPT)
        .await
        .map(|(text, _)| text)
}

/// OCR a single image via Gemini Vision API. Used by `analyze_image` tool
/// when `extract_text` parameter is true.
pub async fn ocr_image_text(
    state: &AppState,
    data_b64: &str,
    mime_type: &str,
) -> Result<String, String> {
    ocr_with_gemini(state, data_b64, mime_type, OCR_PROMPT)
        .await
        .map(|(text, _)| text)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Split OCR output into pages by `---PAGE N---` markers.
fn split_into_pages(text: &str) -> Vec<OcrPage> {
    // Match ---PAGE 2--- (numbered) or ---PAGE N--- (literal N from Gemini)
    let re = regex::Regex::new(r"---PAGE\s+(\d+|N)---").expect("page marker regex is valid");

    let mut pages = Vec::new();
    let mut last_end = 0;
    let mut page_num = 1;

    for cap in re.captures_iter(text) {
        let m = cap.get(0).unwrap();
        let before = text[last_end..m.start()].trim();
        if !before.is_empty() {
            pages.push(OcrPage {
                page_number: page_num,
                text: before.to_string(),
            });
            page_num += 1;
        }
        if let Ok(n) = cap[1].parse::<usize>() {
            page_num = n;
        }
        last_end = m.end();
    }

    // Remaining text after last marker (or entire text if no markers)
    let remaining = text[last_end..].trim();
    if !remaining.is_empty() {
        pages.push(OcrPage {
            page_number: page_num,
            text: remaining.to_string(),
        });
    }

    if pages.is_empty() {
        pages.push(OcrPage {
            page_number: 1,
            text: text.to_string(),
        });
    }

    pages
}

/// Send an SSE progress event.
async fn send_progress(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    status: &str,
    detail: Option<&Value>,
) {
    let mut payload = json!({
        "step": "ocr",
        "status": status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    if let Some(d) = detail {
        payload["detail"] = d.clone();
    }
    let event = Event::default()
        .event("progress")
        .data(payload.to_string());
    if tx.send(Ok(event)).await.is_err() {
        tracing::warn!("OCR SSE {status}: client disconnected");
    }
}
