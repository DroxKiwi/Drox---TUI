//! Tool `web_fetch` — récupère une URL HTTP/HTTPS, extrait le texte.
//!
//! Bornes :
//! - Schemes acceptés : `http`, `https` uniquement.
//! - Max bytes téléchargés : 2 MiB (`MAX_BYTES`).
//! - Timeout : 20 s.
//! - HTML → extraction du texte visible (script/style/noscript écartés).
//! - Autres types → contenu brut tronqué à `MAX_TEXT_CHARS`.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use schemars::JsonSchema;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 32_768;
const HTTP_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WebFetchInput {
    /// URL HTTP/HTTPS à récupérer.
    pub url: String,
}

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Télécharge une URL HTTP(S) et extrait le texte. HTML : balises script/style filtrées. \
         Taille max : 2 MiB, texte renvoyé tronqué à 32 KiB."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(WebFetchInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: WebFetchInput = serde_json::from_value(input)?;
        let url = Url::parse(&args.url)
            .map_err(|e| ToolError::invalid_args(format!("invalid url: {e}")))?;
        match url.scheme() {
            "http" | "https" => {}
            other => {
                return Err(ToolError::invalid_args(format!(
                    "unsupported scheme `{other}` (only http/https allowed)"
                )));
            }
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .user_agent("drox-tools/0.1")
            .build()
            .map_err(|e| ToolError::network(e.to_string()))?;

        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| ToolError::network(e.to_string()))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();

        if !status.is_success() {
            return Err(ToolError::network(format!("HTTP {status} from {url}")));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::network(e.to_string()))?;
        let total_bytes = bytes.len();
        let capped = bytes.slice(0..bytes.len().min(MAX_BYTES));
        let truncated_download = total_bytes > MAX_BYTES;

        let raw_text = String::from_utf8_lossy(&capped).into_owned();
        let (text, kind) = if is_html_like(&content_type) {
            (extract_text_from_html(&raw_text), "html")
        } else {
            (raw_text, "text")
        };

        let (text, truncated_text) = if text.chars().count() > MAX_TEXT_CHARS {
            (text.chars().take(MAX_TEXT_CHARS).collect::<String>(), true)
        } else {
            (text, false)
        };

        Ok(json!({
            "url": url.as_str(),
            "status": status.as_u16(),
            "content_type": content_type,
            "kind": kind,
            "bytes": total_bytes,
            "truncated_download": truncated_download,
            "truncated_text": truncated_text,
            "text": text,
        }))
    }
}

fn is_html_like(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.contains("text/html") || ct.contains("application/xhtml")
}

/// Extraction de texte minimaliste : on parse l'HTML, on écarte `script`,
/// `style`, `noscript` et on concatène les nœuds texte.
fn extract_text_from_html(html: &str) -> String {
    let doc = Html::parse_document(html);
    // Selector erreur impossible sur ces littéraux : on `expect` localement.
    let skip = Selector::parse("script, style, noscript, template").expect("static selector");
    let to_skip: std::collections::HashSet<_> = doc.select(&skip).map(|e| e.id()).collect();

    let mut out = String::new();
    for node in doc.tree.nodes() {
        if let Some(text) = node.value().as_text() {
            // Skip si un ancêtre est dans to_skip.
            let mut cur = node.parent();
            let mut skipped = false;
            while let Some(p) = cur {
                if to_skip.contains(&p.id()) {
                    skipped = true;
                    break;
                }
                cur = p.parent();
            }
            if skipped {
                continue;
            }
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(trimmed);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_extraction_removes_script_and_style() {
        let html = r"
            <html><head><style>.x { color: red; }</style></head>
            <body><p>Bonjour <b>monde</b></p><script>alert(1)</script></body></html>
        ";
        let text = extract_text_from_html(html);
        assert!(text.contains("Bonjour"));
        assert!(text.contains("monde"));
        assert!(!text.contains("color"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn content_type_detection() {
        assert!(is_html_like("text/html; charset=utf-8"));
        assert!(is_html_like("application/xhtml+xml"));
        assert!(!is_html_like("application/json"));
    }
}
