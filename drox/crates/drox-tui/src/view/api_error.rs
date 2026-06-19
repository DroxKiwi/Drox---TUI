//! Erreurs API / LLM structurées (leak : `SystemAPIErrorMessage.tsx`).

use drox_engine::EngineError;
use drox_llm::LlmError;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const MAX_BODY_CHARS: usize = 800;

/// Vue d'une erreur API pour le fil.
#[derive(Debug, Clone)]
pub struct ApiErrorView {
    pub status: Option<u16>,
    pub headline: String,
    pub detail: Option<String>,
    pub retryable: bool,
}

impl ApiErrorView {
    #[must_use]
    pub fn from_engine(err: &EngineError) -> Option<Self> {
        match err {
            EngineError::Llm(llm) => Some(Self::from_llm(llm)),
            _ => None,
        }
    }

    #[must_use]
    pub fn from_llm(err: &LlmError) -> Self {
        match err {
            LlmError::Api { status, body } => Self {
                status: Some(*status),
                headline: status_headline(*status),
                detail: Some(truncate_body(body)),
                retryable: err.is_retryable(),
            },
            LlmError::RetryExhausted { attempts, source } => {
                let inner = Self::from_llm(source);
                Self {
                    status: inner.status,
                    headline: format!("Échec après {attempts} tentative(s)"),
                    detail: Some(format!(
                        "{}",
                        inner.detail.unwrap_or_else(|| inner.headline.clone())
                    )),
                    retryable: false,
                }
            }
            LlmError::Http(e) if e.is_timeout() => Self {
                status: None,
                headline: "Délai dépassé (timeout réseau)".into(),
                detail: Some(e.to_string()),
                retryable: true,
            },
            LlmError::Http(e) => Self {
                status: e.status().map(|s| s.as_u16()),
                headline: "Erreur transport HTTP".into(),
                detail: Some(e.to_string()),
                retryable: err.is_retryable(),
            },
            LlmError::StreamTerminated => Self {
                status: None,
                headline: "Flux LLM interrompu".into(),
                detail: None,
                retryable: true,
            },
            other => Self {
                status: None,
                headline: other.to_string(),
                detail: None,
                retryable: other.is_retryable(),
            },
        }
    }
}

fn status_headline(status: u16) -> String {
    match status {
        401 => "Non autorisé (401)".into(),
        403 => "Accès refusé (403)".into(),
        404 => "Modèle ou endpoint introuvable (404)".into(),
        429 => "Limite de débit (429)".into(),
        500..=599 => format!("Erreur serveur ({status})"),
        other => format!("Erreur API ({other})"),
    }
}

fn truncate_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.len() <= MAX_BODY_CHARS {
        return trimmed.to_string();
    }
    format!("{}…", &trimmed[..MAX_BODY_CHARS])
}

#[must_use]
pub fn render_api_error(view: &ApiErrorView) -> Vec<Line<'static>> {
    let style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let detail_style = Style::default().fg(Color::Red);
    let mut lines = Vec::new();
    let status = view
        .status
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled("✗ API", style),
        Span::styled(status, style),
        Span::styled(format!(" — {}", view.headline), style),
    ]));
    if let Some(ref detail) = view.detail {
        for line in detail.lines().take(12) {
            lines.push(Line::from(vec![
                Span::styled("  ", detail_style),
                Span::styled(line.to_string(), detail_style),
            ]));
        }
    }
    if view.retryable {
        let hint = Style::default().fg(Color::Yellow);
        lines.push(Line::from(Span::styled(
            "  ↻ erreur transitoire — relancez le prompt",
            hint,
        )));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_rate_limit() {
        let err = LlmError::Api {
            status: 429,
            body: "too many requests".into(),
        };
        let view = ApiErrorView::from_llm(&err);
        assert_eq!(view.status, Some(429));
        let lines = render_api_error(&view);
        assert!(lines[0].to_string().contains("429"));
        assert!(lines.iter().any(|l| l.to_string().contains("relancez")));
    }
}
