// backend/src/tools/web_scraping.rs
//! Web scraping tools (v2 — 50 improvements).
//!
//! Provides `fetch_webpage` and `crawl_website` tools with SSRF prevention,
//! robots.txt compliance, sitemap seeding, concurrent crawling, content
//! deduplication (SHA-256), enhanced HTML→markdown extraction, metadata,
//! retry with exponential backoff, and JSON/text output formats.

use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use url::Url;

use super::ToolOutput;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_PAGE_SIZE: usize = 5 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CRAWL_DELAY_MS: u64 = 300;
const MAX_CRAWL_DEPTH: u32 = 5;
const MAX_CRAWL_PAGES: usize = 50;
const MAX_CONCURRENT: usize = 5;
const MAX_TOTAL_CRAWL_SECS: u64 = 180;
const MAX_RETRY_ATTEMPTS: u32 = 3;
const WEB_USER_AGENT: &str = "Jaskier-Bot/1.0 (AI Agent Tool)";

const TRACKING_PARAMS: &[&str] = &[
    "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
    "fbclid", "gclid", "mc_cid", "mc_eid", "ref", "_ga",
];

const SKIP_EXTENSIONS: &[&str] = &[
    ".pdf", ".zip", ".tar", ".gz", ".rar", ".7z",
    ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".ico", ".bmp",
    ".css", ".js", ".woff", ".woff2", ".ttf", ".eot",
    ".xml", ".json", ".rss", ".atom",
    ".mp3", ".mp4", ".avi", ".mov", ".wmv", ".flv",
    ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
    ".exe", ".dmg", ".apk", ".deb", ".rpm",
];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

struct WebFetchResult {
    url: String,
    title: String,
    text: String,
    metadata: WebPageMetadata,
    links: Vec<WebCategorizedLink>,
    content_hash: String,
}

#[derive(Default)]
struct WebPageMetadata {
    description: String,
    og_title: String,
    og_description: String,
    og_image: String,
    canonical_url: String,
    language: String,
    json_ld: Vec<String>,
}

#[derive(Clone)]
enum WebLinkType { Internal, External, Resource }

#[derive(Clone)]
struct WebCategorizedLink {
    href: String,
    anchor: String,
    link_type: WebLinkType,
}

struct WebRobotsRules {
    disallow: Vec<String>,
    allow: Vec<String>,
    crawl_delay: Option<f64>,
    sitemaps: Vec<String>,
}

struct WebPageResult {
    url: String,
    title: String,
    text_excerpt: String,
    content_hash: String,
    metadata: WebPageMetadata,
    links: Vec<WebCategorizedLink>,
}

struct WebExtractionOptions {
    include_links: bool,
    include_metadata: bool,
    include_images: bool,
    max_text_length: usize,
}

// ---------------------------------------------------------------------------
// URL validation, normalization, SSRF prevention
// ---------------------------------------------------------------------------

fn web_validate_url(raw: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw).map_err(|e| format!("Invalid URL '{}': {}", raw, e))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Unsupported scheme '{}' — only http/https", other)),
    }
    // SSRF prevention
    if let Some(host) = parsed.host_str() {
        let lower = host.to_lowercase();
        if lower == "localhost"
            || lower == "metadata.google.internal"
            || lower.ends_with(".internal")
            || lower == "169.254.169.254"
        {
            return Err(format!("Blocked host: {}", host));
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            let is_private = match ip {
                IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
                IpAddr::V6(v6) => v6.is_loopback(),
            };
            if is_private {
                return Err(format!("Blocked private/loopback IP: {}", ip));
            }
        }
    }
    Ok(parsed)
}

fn web_normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    // Strip tracking params
    let pairs: Vec<(String, String)> = normalized
        .query_pairs()
        .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.as_ref()))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    if pairs.is_empty() {
        normalized.set_query(None);
    } else {
        let mut sorted = pairs;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let qs: Vec<String> = sorted.iter().map(|(k, v)| {
            if v.is_empty() { k.clone() } else { format!("{}={}", k, v) }
        }).collect();
        normalized.set_query(Some(&qs.join("&")));
    }
    // Remove trailing slash for non-root paths
    let mut s = normalized.to_string();
    if s.ends_with('/') && normalized.path() != "/" {
        s.pop();
    }
    s
}

fn web_should_skip_url(path: &str) -> bool {
    let lower = path.to_lowercase();
    SKIP_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

// ---------------------------------------------------------------------------
// robots.txt
// ---------------------------------------------------------------------------

async fn web_fetch_robots(client: &reqwest::Client, base: &Url) -> Option<WebRobotsRules> {
    let robots_url = format!("{}://{}/robots.txt", base.scheme(), base.authority());
    let resp = client
        .get(&robots_url)
        .header("User-Agent", WEB_USER_AGENT)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() { return None; }
    let text = resp.text().await.ok()?;
    Some(web_parse_robots(&text))
}

fn web_parse_robots(text: &str) -> WebRobotsRules {
    let mut rules = WebRobotsRules {
        disallow: Vec::new(),
        allow: Vec::new(),
        crawl_delay: None,
        sitemaps: Vec::new(),
    };
    let mut in_our_section = false;
    let mut in_any_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let lower = line.to_lowercase();
        if lower.starts_with("user-agent:") {
            let agent = line[11..].trim().to_lowercase();
            in_any_section = true;
            in_our_section = agent == "*" || agent.contains("jaskier");
        } else if lower.starts_with("sitemap:") {
            let url = line[8..].trim();
            if !url.is_empty() {
                rules.sitemaps.push(url.to_string());
            }
        } else if in_our_section || !in_any_section {
            if lower.starts_with("disallow:") {
                let path = line[9..].trim();
                if !path.is_empty() { rules.disallow.push(path.to_string()); }
            } else if lower.starts_with("allow:") {
                let path = line[6..].trim();
                if !path.is_empty() { rules.allow.push(path.to_string()); }
            } else if lower.starts_with("crawl-delay:") {
                if let Ok(d) = line[12..].trim().parse::<f64>() {
                    rules.crawl_delay = Some(d);
                }
            }
        }
    }
    rules
}

fn web_is_allowed_by_robots(rules: &WebRobotsRules, path: &str) -> bool {
    // Allow rules take precedence over disallow for same-length match
    for a in &rules.allow {
        if path.starts_with(a.as_str()) { return true; }
    }
    for d in &rules.disallow {
        if path.starts_with(d.as_str()) { return false; }
    }
    true
}

// ---------------------------------------------------------------------------
// Sitemap
// ---------------------------------------------------------------------------

async fn web_fetch_sitemap(client: &reqwest::Client, base: &Url, robots: &Option<WebRobotsRules>) -> Vec<String> {
    let mut sitemap_urls: Vec<String> = Vec::new();
    let mut candidates = Vec::new();
    if let Some(r) = robots {
        candidates.extend(r.sitemaps.clone());
    }
    if candidates.is_empty() {
        candidates.push(format!("{}://{}/sitemap.xml", base.scheme(), base.authority()));
    }
    for sm_url in &candidates {
        if let Ok(resp) = client
            .get(sm_url)
            .header("User-Agent", WEB_USER_AGENT)
            .timeout(Duration::from_secs(15))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    web_parse_sitemap_xml(&body, &mut sitemap_urls);
                }
            }
        }
    }
    sitemap_urls
}

fn web_parse_sitemap_xml(xml: &str, out: &mut Vec<String>) {
    // Simple regex-based XML parsing for <loc> tags
    let loc_re = Regex::new(r"<loc>\s*(.*?)\s*</loc>").expect("loc regex is valid");
    let is_index = xml.contains("<sitemapindex");
    for cap in loc_re.captures_iter(xml) {
        if let Some(m) = cap.get(1) {
            let url = m.as_str().trim();
            if !url.is_empty() {
                if is_index {
                    // Sitemap index — these are sub-sitemaps; we just collect the URLs
                    out.push(url.to_string());
                } else {
                    out.push(url.to_string());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP fetch with retry
// ---------------------------------------------------------------------------

async fn web_fetch_with_retry(
    client: &reqwest::Client,
    url: &str,
    custom_headers: &HashMap<String, String>,
) -> Result<(String, Url, u16), String> {
    let parsed = web_validate_url(url)?;
    let mut last_err = String::new();

    for attempt in 0..MAX_RETRY_ATTEMPTS {
        if attempt > 0 {
            let delay = Duration::from_millis(500 * 2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }
        let mut req = client
            .get(parsed.as_str())
            .header("User-Agent", WEB_USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9,pl;q=0.8")
            .timeout(FETCH_TIMEOUT);
        for (k, v) in custom_headers {
            req = req.header(k.as_str(), v.as_str());
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == 429 || (status >= 500 && status < 600) {
                    last_err = format!("HTTP {} for '{}'", status, url);
                    continue; // retry
                }
                if !resp.status().is_success() {
                    return Err(format!("HTTP {} for '{}'", status, url));
                }
                // Content-Type check
                let ct = resp.headers().get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if !ct.is_empty() && !ct.contains("text/") && !ct.contains("html") && !ct.contains("xml") {
                    return Err(format!("Non-HTML content: {} for '{}'", ct, url));
                }
                if let Some(len) = resp.content_length() {
                    if len as usize > MAX_PAGE_SIZE {
                        return Err(format!("Response too large: {} bytes", len));
                    }
                }
                let final_url = resp.url().clone();
                let bytes = resp.bytes().await
                    .map_err(|e| format!("Read body failed for '{}': {}", url, e))?;
                if bytes.len() > MAX_PAGE_SIZE {
                    return Err(format!("Response too large: {} bytes", bytes.len()));
                }
                let body = String::from_utf8_lossy(&bytes).to_string();
                return Ok((body, final_url, status));
            }
            Err(e) => {
                last_err = format!("Fetch '{}': {}", url, e);
                if e.is_timeout() { continue; }
                return Err(last_err);
            }
        }
    }
    Err(format!("Failed after {} retries: {}", MAX_RETRY_ATTEMPTS, last_err))
}

// ---------------------------------------------------------------------------
// Enhanced HTML -> text extraction
// ---------------------------------------------------------------------------

fn web_extract_text(html: &str, opts: &WebExtractionOptions) -> String {
    let doc = Html::parse_document(html);

    // Title
    let title = Selector::parse("title").ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>());

    // Content priority: article > main > body
    let content_el = ["article", "main", "body"].iter()
        .find_map(|tag| {
            Selector::parse(tag).ok()
                .and_then(|sel| doc.select(&sel).next())
        });

    let mut raw = String::new();
    if let Some(el) = content_el {
        web_collect_element_text(el, &mut raw, opts);
    }

    // Collapse whitespace
    let lines: Vec<&str> = raw.lines().map(|l| l.trim_end()).collect();
    let mut output = String::new();
    let mut blank_count = 0;
    for line in lines {
        if line.is_empty() {
            blank_count += 1;
            if blank_count <= 2 { output.push('\n'); }
        } else {
            blank_count = 0;
            output.push_str(line);
            output.push('\n');
        }
    }

    let text = output.trim().to_string();
    if let Some(t) = title {
        let t = t.trim();
        if !t.is_empty() {
            return format!("# {}\n\n{}", t, text);
        }
    }
    text
}

fn web_collect_element_text(element: scraper::ElementRef, out: &mut String, opts: &WebExtractionOptions) {
    let tag = element.value().name();

    // Skip noise
    if matches!(tag, "script" | "style" | "noscript" | "svg" | "iframe" | "nav" | "footer" | "header") {
        return;
    }

    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level: usize = tag[1..].parse().unwrap_or(1);
            let prefix = "#".repeat(level);
            let text: String = element.text().collect::<Vec<_>>().join(" ");
            let text = text.trim();
            if !text.is_empty() {
                out.push_str(&format!("\n{} {}\n\n", prefix, text));
            }
            return; // don't recurse into heading children
        }
        "pre" | "code" => {
            let text: String = element.text().collect::<Vec<_>>().join("");
            if !text.trim().is_empty() {
                // Detect language from class
                let lang = element.value().attr("class")
                    .and_then(|c| c.split_whitespace()
                        .find(|cls| cls.starts_with("language-") || cls.starts_with("lang-"))
                        .map(|cls| cls.split('-').nth(1).unwrap_or("")))
                    .unwrap_or("");
                out.push_str(&format!("\n```{}\n{}\n```\n", lang, text.trim()));
            }
            return;
        }
        "table" => {
            web_extract_table(element, out);
            return;
        }
        "img" if opts.include_images => {
            let alt = element.value().attr("alt").unwrap_or("").trim();
            if !alt.is_empty() {
                let src = element.value().attr("src").unwrap_or("");
                out.push_str(&format!("![{}]({})", alt, src));
            }
            return;
        }
        "a" if opts.include_links => {
            let href = element.value().attr("href").unwrap_or("").trim();
            let text: String = element.text().collect::<Vec<_>>().join(" ");
            let text = text.trim();
            if !text.is_empty() && !href.is_empty()
                && !href.starts_with('#') && !href.starts_with("javascript:")
            {
                out.push_str(&format!("[{}]({})", text, href));
            } else if !text.is_empty() {
                out.push_str(text);
            }
            return;
        }
        "details" => {
            // Expand details/summary
            if let Ok(sum_sel) = Selector::parse("summary") {
                if let Some(summary) = element.select(&sum_sel).next() {
                    let text: String = summary.text().collect::<Vec<_>>().join(" ");
                    out.push_str(&format!("\n**{}**\n", text.trim()));
                }
            }
        }
        "dl" => {
            web_extract_definition_list(element, out);
            return;
        }
        "li" => {
            out.push_str("- ");
        }
        "br" => {
            out.push('\n');
            return;
        }
        "hr" => {
            out.push_str("\n---\n");
            return;
        }
        "p" | "div" | "section" | "article" | "main" | "blockquote" => {
            out.push('\n');
        }
        _ => {}
    }

    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(text) => {
                let t = text.text.trim();
                if !t.is_empty() {
                    out.push_str(t);
                    out.push(' ');
                }
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    web_collect_element_text(child_el, out, opts);
                }
            }
            _ => {}
        }
    }

    if matches!(tag, "p" | "div" | "section" | "article" | "main" | "blockquote" | "li") {
        out.push('\n');
    }
}

fn web_extract_table(table: scraper::ElementRef, out: &mut String) {
    let row_sel = Selector::parse("tr").expect("tr selector is valid");
    let th_sel = Selector::parse("th").expect("th selector is valid");
    let td_sel = Selector::parse("td").expect("td selector is valid");

    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in table.select(&row_sel) {
        let mut cells: Vec<String> = Vec::new();
        for cell in row.select(&th_sel).chain(row.select(&td_sel)) {
            let text: String = cell.text().collect::<Vec<_>>().join(" ");
            cells.push(text.trim().replace('|', "\\|").to_string());
        }
        if !cells.is_empty() { rows.push(cells); }
    }
    if rows.is_empty() { return; }

    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    out.push('\n');
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for j in 0..max_cols {
            let cell = row.get(j).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(" {} |", cell));
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in 0..max_cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

fn web_extract_definition_list(dl: scraper::ElementRef, out: &mut String) {
    let dt_sel = Selector::parse("dt").expect("dt selector is valid");
    let dd_sel = Selector::parse("dd").expect("dd selector is valid");
    out.push('\n');
    for dt in dl.select(&dt_sel) {
        let text: String = dt.text().collect::<Vec<_>>().join(" ");
        out.push_str(&format!("**{}**\n", text.trim()));
    }
    for dd in dl.select(&dd_sel) {
        let text: String = dd.text().collect::<Vec<_>>().join(" ");
        out.push_str(&format!(": {}\n", text.trim()));
    }
    out.push('\n');
}

// ---------------------------------------------------------------------------
// Metadata extraction
// ---------------------------------------------------------------------------

fn web_extract_metadata(html: &str, base_url: &Url) -> WebPageMetadata {
    let doc = Html::parse_document(html);
    let mut meta = WebPageMetadata::default();

    if let Ok(sel) = Selector::parse("meta") {
        for el in doc.select(&sel) {
            let name = el.value().attr("name").or_else(|| el.value().attr("property")).unwrap_or("");
            let content = el.value().attr("content").unwrap_or("");
            match name {
                "description" => meta.description = content.to_string(),
                "og:title" => meta.og_title = content.to_string(),
                "og:description" => meta.og_description = content.to_string(),
                "og:image" => meta.og_image = content.to_string(),
                _ => {}
            }
        }
    }
    if let Ok(sel) = Selector::parse("link[rel='canonical']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                meta.canonical_url = href.to_string();
            }
        }
    }
    if let Ok(sel) = Selector::parse("html") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.value().attr("lang") {
                meta.language = lang.to_string();
            }
        }
    }
    // JSON-LD
    if let Ok(sel) = Selector::parse("script[type='application/ld+json']") {
        for el in doc.select(&sel) {
            let text: String = el.text().collect();
            let text = text.trim();
            if !text.is_empty() && text.len() < 5000 {
                meta.json_ld.push(text.to_string());
            }
        }
    }
    let _ = base_url; // reserved for future relative URL resolution in metadata
    meta
}

// ---------------------------------------------------------------------------
// Link extraction & categorization
// ---------------------------------------------------------------------------

fn web_extract_links(html: &str, base_url: &Url) -> Vec<WebCategorizedLink> {
    let doc = Html::parse_document(html);
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let base_domain = base_url.domain().unwrap_or("");

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in doc.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let href = href.trim();
                if href.is_empty()
                    || href.starts_with('#')
                    || href.starts_with("javascript:")
                    || href.starts_with("mailto:")
                    || href.starts_with("tel:")
                    || href.starts_with("data:")
                {
                    continue;
                }
                let resolved = match base_url.join(href) {
                    Ok(u) => web_normalize_url(&u),
                    Err(_) => continue,
                };
                if seen.contains(&resolved) { continue; }
                seen.insert(resolved.clone());

                let anchor: String = el.text().collect::<Vec<_>>().join(" ");
                let anchor = anchor.trim().to_string();

                let link_type = if web_should_skip_url(&resolved) {
                    WebLinkType::Resource
                } else if let Ok(u) = Url::parse(&resolved) {
                    if u.domain().unwrap_or("") == base_domain {
                        WebLinkType::Internal
                    } else {
                        WebLinkType::External
                    }
                } else {
                    WebLinkType::External
                };

                links.push(WebCategorizedLink { href: resolved, anchor, link_type });
            }
        }
    }
    links
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn web_content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn web_truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len { return text.to_string(); }
    text.char_indices()
        .take_while(|(i, _)| *i < max_len)
        .map(|(_, c)| c)
        .collect::<String>()
        + "\u{2026}"
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

fn web_format_fetch_output(result: &WebFetchResult, opts: &WebExtractionOptions, as_json: bool) -> String {
    if as_json {
        let links_json: Vec<Value> = if opts.include_links {
            result.links.iter().map(|l| json!({
                "href": l.href,
                "anchor": l.anchor,
                "type": match l.link_type { WebLinkType::Internal => "internal", WebLinkType::External => "external", WebLinkType::Resource => "resource" },
            })).collect()
        } else { vec![] };
        let mut obj = json!({
            "url": result.url,
            "title": result.title,
            "content_hash": result.content_hash,
            "text": web_truncate_text(&result.text, opts.max_text_length),
        });
        if opts.include_links { obj["links"] = json!(links_json); }
        if opts.include_metadata {
            obj["metadata"] = json!({
                "description": result.metadata.description,
                "og_title": result.metadata.og_title,
                "og_description": result.metadata.og_description,
                "og_image": result.metadata.og_image,
                "canonical_url": result.metadata.canonical_url,
                "language": result.metadata.language,
            });
        }
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| format!("{:?}", obj))
    } else {
        let mut out = format!("## {}\n**URL**: {}\n**Hash**: {}\n\n",
            result.title, result.url, result.content_hash);
        if opts.include_metadata {
            let m = &result.metadata;
            if !m.description.is_empty() { out.push_str(&format!("**Description**: {}\n", m.description)); }
            if !m.language.is_empty() { out.push_str(&format!("**Language**: {}\n", m.language)); }
            if !m.canonical_url.is_empty() { out.push_str(&format!("**Canonical**: {}\n", m.canonical_url)); }
            out.push('\n');
        }
        out.push_str(&web_truncate_text(&result.text, opts.max_text_length));
        if opts.include_links && !result.links.is_empty() {
            out.push_str("\n\n---\n### Links\n\n");
            let internal: Vec<_> = result.links.iter().filter(|l| matches!(l.link_type, WebLinkType::Internal)).collect();
            let external: Vec<_> = result.links.iter().filter(|l| matches!(l.link_type, WebLinkType::External)).collect();
            if !internal.is_empty() {
                out.push_str(&format!("**Internal ({}):**\n", internal.len()));
                for l in &internal {
                    let label = if l.anchor.is_empty() { &l.href } else { &l.anchor };
                    out.push_str(&format!("- [{}]({})\n", label, l.href));
                }
            }
            if !external.is_empty() {
                out.push_str(&format!("\n**External ({}):**\n", external.len()));
                for l in &external {
                    let label = if l.anchor.is_empty() { &l.href } else { &l.anchor };
                    out.push_str(&format!("- [{}]({})\n", label, l.href));
                }
            }
        }
        out
    }
}

fn web_format_crawl_output(
    results: &[WebPageResult],
    errors: &[String],
    start_url: &str,
    elapsed_secs: f64,
    as_json: bool,
) -> String {
    let total_links: usize = results.iter().map(|r| r.links.len()).sum();
    if as_json {
        let pages: Vec<Value> = results.iter().map(|r| {
            let links: Vec<Value> = r.links.iter().map(|l| json!({
                "href": l.href, "anchor": l.anchor,
                "type": match l.link_type { WebLinkType::Internal => "internal", WebLinkType::External => "external", WebLinkType::Resource => "resource" },
            })).collect();
            let mut page = json!({
                "url": r.url, "title": r.title, "content_hash": r.content_hash,
                "text_excerpt": r.text_excerpt, "links": links,
            });
            if !r.metadata.language.is_empty() || !r.metadata.description.is_empty() {
                page["metadata"] = json!({
                    "description": r.metadata.description,
                    "language": r.metadata.language,
                    "canonical_url": r.metadata.canonical_url,
                });
            }
            page
        }).collect();
        let obj = json!({
            "start_url": start_url,
            "pages_fetched": results.len(),
            "total_links": total_links,
            "errors": errors.len(),
            "elapsed_seconds": (elapsed_secs * 10.0).round() / 10.0,
            "pages": pages,
            "crawl_errors": errors,
        });
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".to_string())
    } else {
        let mut out = format!(
            "## Crawl: {}\n**Pages**: {} | **Links**: {} | **Errors**: {} | **Time**: {:.1}s\n\n",
            start_url, results.len(), total_links, errors.len(), elapsed_secs
        );
        for (i, r) in results.iter().enumerate() {
            out.push_str(&format!("### {}. {} ({})\n{}\n\n", i + 1, r.title, r.url, r.text_excerpt));
        }
        if !results.is_empty() {
            out.push_str("---\n### Link Index\n\n");
            for r in results {
                for l in &r.links {
                    if matches!(l.link_type, WebLinkType::Internal) {
                        let label = if l.anchor.is_empty() { &l.href } else { &l.anchor };
                        out.push_str(&format!("- [{}]({}) \u{2190} {}\n", label, l.href, r.url));
                    }
                }
            }
        }
        if !errors.is_empty() {
            out.push_str("\n---\n### Errors\n\n");
            for e in errors { out.push_str(&format!("- {}\n", e)); }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Tool entry points (called from mod.rs dispatch)
// ---------------------------------------------------------------------------

pub(crate) async fn tool_fetch_webpage(args: &Value, client: &reqwest::Client) -> Result<ToolOutput, String> {
    let url = args["url"].as_str().ok_or("Missing 'url'")?;
    let extract_links = args["extract_links"].as_bool().unwrap_or(true);
    let extract_metadata = args["extract_metadata"].as_bool().unwrap_or(false);
    let include_images = args["include_images"].as_bool().unwrap_or(false);
    let output_format = args["output_format"].as_str().unwrap_or("text");
    let max_text_length = args["max_text_length"].as_u64().unwrap_or(0) as usize;
    let custom_headers: HashMap<String, String> = args["headers"].as_object()
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default();

    let (html, final_url, _status) = web_fetch_with_retry(client, url, &custom_headers).await?;

    let opts = WebExtractionOptions {
        include_links: extract_links,
        include_metadata: extract_metadata,
        include_images,
        max_text_length: if max_text_length == 0 { usize::MAX } else { max_text_length },
    };

    let text = web_extract_text(&html, &opts);
    let title_sel = Selector::parse("title").ok()
        .and_then(|sel| Html::parse_document(&html).select(&sel).next()
            .map(|el| el.text().collect::<String>()));
    let title = title_sel.unwrap_or_default().trim().to_string();
    let content_hash = web_content_hash(&text);
    let metadata = if extract_metadata { web_extract_metadata(&html, &final_url) } else { WebPageMetadata::default() };
    let links = if extract_links { web_extract_links(&html, &final_url) } else { Vec::new() };

    let result = WebFetchResult {
        url: final_url.to_string(),
        title,
        text,
        metadata,
        links,
        content_hash,
    };

    let output = web_format_fetch_output(&result, &opts, output_format == "json");
    Ok(ToolOutput::text(output))
}

pub(crate) async fn tool_crawl_website(args: &Value, client: &reqwest::Client) -> Result<ToolOutput, String> {
    let start_url = args["url"].as_str().ok_or("Missing 'url'")?;
    let max_depth = (args["max_depth"].as_u64().unwrap_or(1) as u32).min(MAX_CRAWL_DEPTH);
    let max_pages = (args["max_pages"].as_u64().unwrap_or(10) as usize).min(MAX_CRAWL_PAGES);
    let same_domain = args["same_domain_only"].as_bool().unwrap_or(true);
    let path_prefix = args["path_prefix"].as_str().unwrap_or("");
    let respect_robots = args["respect_robots_txt"].as_bool().unwrap_or(true);
    let use_sitemap = args["use_sitemap"].as_bool().unwrap_or(false);
    let concurrent = (args["concurrent_requests"].as_u64().unwrap_or(1) as usize).min(MAX_CONCURRENT);
    let delay_ms = args["delay_ms"].as_u64().unwrap_or(DEFAULT_CRAWL_DELAY_MS);
    let max_total_secs = (args["max_total_seconds"].as_u64().unwrap_or(MAX_TOTAL_CRAWL_SECS)).min(MAX_TOTAL_CRAWL_SECS);
    let output_format = args["output_format"].as_str().unwrap_or("text");
    let max_text_length = args["max_text_length"].as_u64().unwrap_or(2000) as usize;
    let include_metadata = args["include_metadata"].as_bool().unwrap_or(false);
    let custom_headers: HashMap<String, String> = args["headers"].as_object()
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default();
    let exclude_patterns: Vec<String> = args["exclude_patterns"].as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let start_parsed = web_validate_url(start_url)?;
    let start_domain = start_parsed.domain().unwrap_or("").to_string();
    let started = Instant::now();

    // Robots.txt
    let robots = if respect_robots {
        web_fetch_robots(client, &start_parsed).await
    } else {
        None
    };

    // Effective crawl delay
    let effective_delay = if let Some(ref r) = robots {
        r.crawl_delay.map(|d| (d * 1000.0) as u64).unwrap_or(delay_ms)
    } else {
        delay_ms
    };

    // Seed queue
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut results: Vec<WebPageResult> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut content_hashes: HashSet<String> = HashSet::new();

    // Sitemap seeding
    if use_sitemap {
        let sitemap_urls = web_fetch_sitemap(client, &start_parsed, &robots).await;
        for su in sitemap_urls {
            if !path_prefix.is_empty() {
                if let Ok(u) = Url::parse(&su) {
                    if !u.path().starts_with(path_prefix) { continue; }
                }
            }
            queue.push_back((su, 0));
        }
    }

    queue.push_back((start_parsed.to_string(), 0));

    let opts = WebExtractionOptions {
        include_links: true,
        include_metadata,
        include_images: false,
        max_text_length,
    };

    while !queue.is_empty() && results.len() < max_pages {
        if started.elapsed().as_secs() > max_total_secs { break; }

        // Batch up to `concurrent` URLs
        let mut batch: Vec<(String, u32)> = Vec::new();
        while batch.len() < concurrent {
            if let Some((url, depth)) = queue.pop_front() {
                let normalized = if let Ok(u) = Url::parse(&url) { web_normalize_url(&u) } else { url.clone() };
                if visited.contains(&normalized) { continue; }
                visited.insert(normalized.clone());
                batch.push((normalized, depth));
            } else {
                break;
            }
        }
        if batch.is_empty() { break; }

        if concurrent > 1 && batch.len() > 1 {
            // Concurrent fetch
            let mut join_set = JoinSet::new();
            for (url, depth) in batch {
                let client = client.clone();
                let headers = custom_headers.clone();
                join_set.spawn(async move {
                    let result = web_fetch_with_retry(&client, &url, &headers).await;
                    (url, depth, result)
                });
            }
            while let Some(res) = join_set.join_next().await {
                if let Ok((url, depth, fetch_result)) = res {
                    match fetch_result {
                        Ok((html, final_url, _)) => {
                            let pr = web_process_page(&html, &final_url, &url, max_text_length, &opts);
                            if content_hashes.contains(&pr.content_hash) { continue; }
                            content_hashes.insert(pr.content_hash.clone());

                            // Enqueue discovered links
                            if depth < max_depth {
                                for link in &pr.links {
                                    if !matches!(link.link_type, WebLinkType::Internal) && same_domain { continue; }
                                    if web_should_skip_url(&link.href) { continue; }
                                    if !path_prefix.is_empty() {
                                        if let Ok(u) = Url::parse(&link.href) {
                                            if !u.path().starts_with(path_prefix) { continue; }
                                        }
                                    }
                                    if !exclude_patterns.is_empty() && exclude_patterns.iter().any(|p| link.href.contains(p)) { continue; }
                                    if same_domain {
                                        if let Ok(u) = Url::parse(&link.href) {
                                            if u.domain().unwrap_or("") != start_domain { continue; }
                                        }
                                    }
                                    if let Some(ref r) = robots {
                                        if let Ok(u) = Url::parse(&link.href) {
                                            if !web_is_allowed_by_robots(r, u.path()) { continue; }
                                        }
                                    }
                                    queue.push_back((link.href.clone(), depth + 1));
                                }
                            }
                            results.push(pr);
                        }
                        Err(e) => errors.push(e),
                    }
                }
            }
        } else {
            // Sequential fetch
            for (url, depth) in batch {
                if started.elapsed().as_secs() > max_total_secs || results.len() >= max_pages { break; }

                if let Some(ref r) = robots {
                    if let Ok(u) = Url::parse(&url) {
                        if !web_is_allowed_by_robots(r, u.path()) { continue; }
                    }
                }

                match web_fetch_with_retry(client, &url, &custom_headers).await {
                    Ok((html, final_url, _)) => {
                        let pr = web_process_page(&html, &final_url, &url, max_text_length, &opts);
                        if content_hashes.contains(&pr.content_hash) { continue; }
                        content_hashes.insert(pr.content_hash.clone());

                        if depth < max_depth {
                            for link in &pr.links {
                                if !matches!(link.link_type, WebLinkType::Internal) && same_domain { continue; }
                                if web_should_skip_url(&link.href) { continue; }
                                if !path_prefix.is_empty() {
                                    if let Ok(u) = Url::parse(&link.href) {
                                        if !u.path().starts_with(path_prefix) { continue; }
                                    }
                                }
                                if !exclude_patterns.is_empty() && exclude_patterns.iter().any(|p| link.href.contains(p)) { continue; }
                                if same_domain {
                                    if let Ok(u) = Url::parse(&link.href) {
                                        if u.domain().unwrap_or("") != start_domain { continue; }
                                    }
                                }
                                if let Some(ref r) = robots {
                                    if let Ok(u) = Url::parse(&link.href) {
                                        if !web_is_allowed_by_robots(r, u.path()) { continue; }
                                    }
                                }
                                queue.push_back((link.href.clone(), depth + 1));
                            }
                        }
                        results.push(pr);
                    }
                    Err(e) => errors.push(e),
                }

                if effective_delay > 0 {
                    tokio::time::sleep(Duration::from_millis(effective_delay)).await;
                }
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    let output = web_format_crawl_output(&results, &errors, start_url, elapsed, output_format == "json");
    Ok(ToolOutput::text(output))
}

fn web_process_page(
    html: &str,
    final_url: &Url,
    _original_url: &str,
    max_text_length: usize,
    opts: &WebExtractionOptions,
) -> WebPageResult {
    let text = web_extract_text(html, opts);
    let title = Selector::parse("title").ok()
        .and_then(|sel| Html::parse_document(html).select(&sel).next()
            .map(|el| el.text().collect::<String>()))
        .unwrap_or_default()
        .trim()
        .to_string();
    let content_hash = web_content_hash(&text);
    let excerpt = web_truncate_text(&text, max_text_length);
    let metadata = if opts.include_metadata { web_extract_metadata(html, final_url) } else { WebPageMetadata::default() };
    let links = web_extract_links(html, final_url);

    WebPageResult { url: final_url.to_string(), title, text_excerpt: excerpt, content_hash, metadata, links }
}
