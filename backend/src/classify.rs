// ---------------------------------------------------------------------------
// classify.rs — Agent classification logic (extracted from handlers/mod.rs)
// ---------------------------------------------------------------------------

use crate::models::WitcherAgent;

/// Remove Polish diacritics for keyword matching.
pub fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

/// Match a keyword against text. Keywords >= 4 chars use substring match;
/// shorter keywords require whole-word match.
pub fn keyword_match(text: &str, keyword: &str) -> bool {
    if keyword.len() >= 4 {
        text.contains(keyword)
    } else {
        text.split(|c: char| !c.is_alphanumeric())
            .any(|word| word == keyword)
    }
}

/// Compute the raw keyword confidence score for a single agent against a prompt.
pub fn classify_agent_score(lower_prompt: &str, agent: &WitcherAgent) -> f64 {
    let mut score = 0.0_f64;
    for keyword in &agent.keywords {
        if keyword_match(lower_prompt, keyword) {
            let weight = if keyword.len() >= 8 {
                2.0
            } else if keyword.len() >= 5 {
                1.5
            } else {
                1.0
            };
            score += weight;
        }
    }
    if score > 0.0 {
        (0.6 + (score / 8.0).min(0.35)).min(0.95)
    } else {
        0.0
    }
}

/// Expert agent classification based on prompt analysis and agent keywords.
pub fn classify_prompt(prompt: &str, agents: &[WitcherAgent]) -> (String, f64, String) {
    let lower = strip_diacritics(&prompt.to_lowercase());
    let mut best: Option<(String, f64, f64, String)> = None;

    for agent in agents {
        let mut score = 0.0_f64;
        let mut matched: Vec<&str> = Vec::new();
        for keyword in &agent.keywords {
            if keyword_match(&lower, keyword) {
                let weight = if keyword.len() >= 8 {
                    2.0
                } else if keyword.len() >= 5 {
                    1.5
                } else {
                    1.0
                };
                score += weight;
                matched.push(keyword);
            }
        }
        if score > 0.0 {
            let confidence = (0.6 + (score / 8.0).min(0.35)).min(0.95);
            let reasoning = format!(
                "Matched [{}] for {} (score: {:.1})",
                matched.join(", "),
                agent.name,
                score
            );
            if best.as_ref().is_none_or(|b| score > b.2) {
                best = Some((agent.id.clone(), confidence, score, reasoning));
            }
        }
    }

    best.map(|(id, conf, _, reason)| (id, conf, reason))
        .unwrap_or_else(|| {
            (
                "eskel".to_string(),
                0.4,
                "Defaulting to Backend & APIs".to_string(),
            )
        })
}

/// Semantic classification fallback via Gemini Flash.
/// Called when keyword-based classification gives low confidence (<0.65).
pub async fn classify_with_gemini(
    client: &reqwest::Client,
    api_key: &str,
    is_oauth: bool,
    prompt: &str,
    agents: &[WitcherAgent],
) -> Option<(String, f64, String)> {
    let agent_list: String = agents
        .iter()
        .map(|a| {
            format!(
                "- {} ({}): {} [keywords: {}]",
                a.id,
                a.role,
                a.description,
                a.keywords.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Safe UTF-8 truncation to 500 chars
    let truncated_prompt: String = prompt
        .char_indices()
        .take_while(|(i, _)| *i < 500)
        .map(|(_, c)| c)
        .collect();

    let classification_prompt = format!(
        "Given this user prompt:\n\"{}\"\n\nWhich agent should handle it? Choose from:\n{}\n\nRespond with ONLY the agent id (lowercase, one word).",
        truncated_prompt, agent_list
    );

    let url =
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";
    let body = serde_json::json!({
        "contents": [{"parts": [{"text": classification_prompt}]}],
        "generationConfig": {"temperature": 1.0, "maxOutputTokens": 256}
    });

    let resp = crate::oauth::apply_google_auth(client.post(url), api_key, is_oauth)
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;

    let j: serde_json::Value = resp.json().await.ok()?;
    let text = j
        .get("candidates")?
        .get(0)?
        .get("content")?
        .get("parts")?
        .get(0)?
        .get("text")?
        .as_str()?;
    let agent_id = text.trim().to_lowercase();

    if agents.iter().any(|a| a.id == agent_id) {
        Some((
            agent_id.clone(),
            0.80,
            format!("Gemini Flash classified as '{}'", agent_id),
        ))
    } else {
        tracing::debug!(
            "classify_with_gemini: Gemini returned unknown agent '{}'",
            agent_id
        );
        None
    }
}
