//! Tool `web_search` — recherche web en langage naturel.
//!
//! Provider initial : **`DuckDuckGo` HTML** (`https://html.duckduckgo.com/html/`),
//! choisi pour la mise en service immédiate (zéro setup, pas de clé API).
//! Le rendu HTML « no-JS » de DDG est volontairement stable, mais reste
//! sensible aux changements de classes CSS — d'où une heuristique tolérante
//! côté parser (plusieurs sélecteurs candidats par champ).
//!
//! Bornes :
//! - Timeout : 15 s.
//! - Max résultats demandables : 25 (cap dur côté tool).
//! - Snippets tronqués à 500 caractères pour préserver le contexte LLM.
//!
//! Distinct de `web_fetch` : `web_fetch` télécharge **une URL connue**,
//! `web_search` produit **une liste de candidats** (titre, URL, extrait).

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use schemars::JsonSchema;
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

const HTTP_TIMEOUT_SECS: u64 = 15;
const DEFAULT_MAX_RESULTS: usize = 10;
const HARD_MAX_RESULTS: usize = 25;
const SNIPPET_CHARS: usize = 500;
const DDG_ENDPOINT: &str = "https://html.duckduckgo.com/html/";
/// User-Agent réaliste : sans ça, DDG renvoie une page d'erreur ou un
/// captcha de type "anomaly".
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WebSearchInput {
    /// Requête en langage naturel.
    pub query: String,
    /// Nombre maximum de résultats à renvoyer (défaut 10, max 25).
    #[serde(default)]
    pub max_results: Option<usize>,
}

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Recherche web (DuckDuckGo) à partir d'une requête en langage naturel. \
         Renvoie une liste de résultats { title, url, snippet }. \
         Utilise `web_fetch` ensuite pour lire le contenu d'une URL choisie."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(WebSearchInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: WebSearchInput = serde_json::from_value(input)?;
        let query = args.query.trim();
        if query.is_empty() {
            return Err(ToolError::invalid_args("query must not be empty"));
        }
        let want = args
            .max_results
            .unwrap_or(DEFAULT_MAX_RESULTS)
            .clamp(1, HARD_MAX_RESULTS);

        let html = fetch_ddg_html(query).await?;
        let mut results = parse_ddg_results(&html);
        let total = results.len();
        let truncated = total > want;
        if truncated {
            results.truncate(want);
        }

        Ok(json!({
            "query": query,
            "provider": "duckduckgo",
            "result_count": results.len(),
            "total_parsed": total,
            "truncated": truncated,
            "results": results,
        }))
    }
}

async fn fetch_ddg_html(query: &str) -> Result<String, ToolError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| ToolError::network(e.to_string()))?;

    let response = client
        .post(DDG_ENDPOINT)
        .header("Referer", "https://duckduckgo.com/")
        .form(&[("q", query), ("kl", "wt-wt"), ("b", "")])
        .send()
        .await
        .map_err(|e| ToolError::network(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(ToolError::network(format!(
            "HTTP {status} from DuckDuckGo HTML endpoint"
        )));
    }
    response
        .text()
        .await
        .map_err(|e| ToolError::network(e.to_string()))
}

/// Représentation interne d'un résultat parsé. Sérialisé tel quel.
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Parse une page DDG HTML "no-JS" et extrait les résultats organiques.
/// Ignore les pubs (`.result--ad`) et résultats vides.
fn parse_ddg_results(html: &str) -> Vec<SearchResult> {
    // Sélecteurs statiques validés une fois (panics impossibles sur ces
    // littéraux — couvert par tests).
    let result_sel = Selector::parse("div.result").expect("static selector");
    let ad_class = "result--ad";
    let title_sels = [
        "h2.result__title a.result__a",
        "h2.result__title a",
        "a.result__a",
    ]
    .map(|s| Selector::parse(s).expect("static selector"));
    let snippet_sels = ["a.result__snippet", "div.result__snippet"]
        .map(|s| Selector::parse(s).expect("static selector"));

    let doc = Html::parse_document(html);
    let mut out = Vec::new();

    for result in doc.select(&result_sel) {
        if result
            .value()
            .attr("class")
            .is_some_and(|c| c.split_ascii_whitespace().any(|t| t == ad_class))
        {
            continue;
        }

        let Some(anchor) = title_sels.iter().find_map(|sel| result.select(sel).next()) else {
            continue;
        };
        let title = element_text(&anchor);
        let raw_href = anchor.value().attr("href").unwrap_or("").to_string();
        let url = canonicalize_ddg_url(&raw_href);
        if url.is_empty() || title.is_empty() {
            continue;
        }

        let snippet_text = snippet_sels
            .iter()
            .find_map(|sel| result.select(sel).next())
            .map(|e| element_text(&e))
            .unwrap_or_default();
        let snippet = truncate_chars(&snippet_text, SNIPPET_CHARS);

        out.push(SearchResult {
            title,
            url,
            snippet,
        });
    }

    out
}

/// Concatène les nœuds texte d'un élément, en compactant les espaces.
fn element_text(el: &ElementRef<'_>) -> String {
    let mut out = String::new();
    for chunk in el.text() {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(trimmed);
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut buf: String = s.chars().take(max).collect();
    buf.push('…');
    buf
}

/// DDG enveloppe les URLs cibles dans des redirects :
///   `//duckduckgo.com/l/?uddg=<encoded>&rut=...`
/// On déballe `uddg` pour exposer l'URL d'origine au modèle. Si `href` est
/// déjà direct, on le renvoie tel quel (en ajoutant `https:` aux URLs
/// protocoles-relatives `//host/...`).
fn canonicalize_ddg_url(href: &str) -> String {
    if href.is_empty() {
        return String::new();
    }
    let normalized = href
        .strip_prefix("//")
        .map_or_else(|| href.to_owned(), |rest| format!("https://{rest}"));

    let Ok(parsed) = url::Url::parse(&normalized) else {
        return normalized;
    };
    let is_ddg_redirect = parsed
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case("duckduckgo.com"))
        && parsed.path().starts_with("/l/");
    if !is_ddg_redirect {
        return normalized;
    }
    parsed
        .query_pairs()
        .find(|(k, _)| k == "uddg")
        .map(|(_, v)| v.into_owned())
        .unwrap_or(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fragment HTML inspiré du rendu réel de `html.duckduckgo.com`. On garde
    /// la structure minimale : `div.result` > `h2.result__title > a.result__a`
    /// + `a.result__snippet`.
    const FIXTURE: &str = r#"
<html><body>
<div class="result result--ad">
  <h2 class="result__title">
    <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fads.example.com%2F&rut=xx">Ad title</a>
  </h2>
  <a class="result__snippet" href="...">Ad snippet</a>
</div>
<div class="result results_links">
  <h2 class="result__title">
    <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdoc.rust-lang.org%2Fbook%2F&rut=abc">The Rust Programming Language</a>
  </h2>
  <a class="result__snippet" href="https://doc.rust-lang.org/book/">A book about <b>Rust</b>, the systems programming language.</a>
</div>
<div class="result results_links">
  <h2 class="result__title">
    <a class="result__a" href="https://crates.io/">crates.io: Rust Package Registry</a>
  </h2>
  <div class="result__snippet">The Rust community's crate registry.</div>
</div>
<div class="result results_links">
  <h2 class="result__title">
    <a class="result__a" href=""></a>
  </h2>
</div>
</body></html>
"#;

    #[test]
    fn parser_skips_ads_and_empty_results() {
        let r = parse_ddg_results(FIXTURE);
        assert_eq!(r.len(), 2, "expected 2 organic results, got {r:?}");
    }

    #[test]
    fn parser_decodes_ddg_uddg_redirect() {
        let r = parse_ddg_results(FIXTURE);
        let first = &r[0];
        assert_eq!(first.url, "https://doc.rust-lang.org/book/");
        assert!(first.title.contains("Rust"));
        assert!(first.snippet.contains("systems programming"));
    }

    #[test]
    fn parser_keeps_direct_https_url() {
        let r = parse_ddg_results(FIXTURE);
        let second = &r[1];
        assert_eq!(second.url, "https://crates.io/");
        assert_eq!(second.snippet, "The Rust community's crate registry.");
    }

    #[test]
    fn canonicalize_passes_https_through() {
        assert_eq!(
            canonicalize_ddg_url("https://example.com/x"),
            "https://example.com/x"
        );
    }

    #[test]
    fn canonicalize_adds_https_to_protocol_relative() {
        assert_eq!(
            canonicalize_ddg_url("//example.com/x"),
            "https://example.com/x"
        );
    }

    #[test]
    fn canonicalize_decodes_uddg_param() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%20b%3Fq%3D1&rut=z";
        assert_eq!(canonicalize_ddg_url(href), "https://example.com/a b?q=1");
    }

    #[test]
    fn canonicalize_returns_empty_on_empty_input() {
        assert_eq!(canonicalize_ddg_url(""), "");
    }

    #[test]
    fn truncate_chars_handles_unicode() {
        let s = "café".repeat(200);
        let out = truncate_chars(&s, 10);
        let count = out.chars().count();
        assert_eq!(count, 11, "10 chars + ellipsis, got {out:?} ({count})");
        assert!(out.ends_with('…'));
    }

    #[tokio::test]
    async fn tool_rejects_empty_query() {
        let tool = WebSearchTool;
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("/tmp"), false);
        let err = tool
            .execute(&ctx, json!({ "query": "   " }))
            .await
            .expect_err("empty query must be rejected");
        assert!(
            matches!(err, ToolError::InvalidArgs(_)),
            "expected InvalidArgs, got {err:?}"
        );
    }
}
