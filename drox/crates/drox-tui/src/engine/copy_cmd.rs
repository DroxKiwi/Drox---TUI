//! `/copy` — copie la dernière réponse assistant (presse-papiers + fichier secours).

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::Context;
use camino::Utf8PathBuf;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

use crate::view::LogEntry;

const MAX_LOOKBACK: usize = 20;
const RESPONSE_FILENAME: &str = "response.md";

/// Bloc de code extrait d'une réponse markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyCodeBlock {
    pub code: String,
    pub lang: Option<String>,
}

/// Entrée du sélecteur `/copy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyChoice {
    FullResponse,
    CodeBlock(usize),
    AlwaysFullResponse,
}

/// Plan d'exécution après parsing de `/copy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyPlan {
    Immediate { text: String, filename: String },
    Picker {
        full_text: String,
        blocks: Vec<CopyCodeBlock>,
    },
    Error(String),
}

/// Textes assistant récents (index 0 = plus récent).
#[must_use]
pub fn collect_recent_assistant_texts(entries: &[LogEntry]) -> Vec<String> {
    let mut texts = Vec::new();
    for entry in entries.iter().rev() {
        if texts.len() >= MAX_LOOKBACK {
            break;
        }
        let LogEntry::Assistant { text } = entry else {
            continue;
        };
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            texts.push(text.clone());
        }
    }
    texts
}

/// Prépare la copie : immédiate ou sélecteur de blocs.
#[must_use]
pub fn prepare_copy(
    entries: &[LogEntry],
    age: usize,
    copy_full_response: bool,
) -> CopyPlan {
    let texts = collect_recent_assistant_texts(entries);
    if texts.is_empty() {
        return CopyPlan::Error("Aucun message assistant à copier.".into());
    }
    if age >= texts.len() {
        return CopyPlan::Error(format!(
            "Seulement {} message(s) assistant disponible(s).",
            texts.len()
        ));
    }
    let text = texts[age].clone();
    let blocks = extract_code_blocks(&text);
    if blocks.is_empty() || copy_full_response {
        return CopyPlan::Immediate {
            text,
            filename: RESPONSE_FILENAME.into(),
        };
    }
    CopyPlan::Picker {
        full_text: text,
        blocks,
    }
}

/// Extrait les blocs fenced d'un markdown.
#[must_use]
pub fn extract_code_blocks(markdown: &str) -> Vec<CopyCodeBlock> {
    let mut blocks = Vec::new();
    let mut in_code = false;
    let mut lang: Option<String> = None;
    let mut buf = String::new();

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = true;
                buf.clear();
                lang = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(l) => {
                        let s = l.to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    }
                };
            }
            Event::Text(t) if in_code => buf.push_str(&t),
            Event::Code(t) if in_code => buf.push_str(&t),
            Event::End(TagEnd::CodeBlock) if in_code => {
                blocks.push(CopyCodeBlock {
                    code: buf.clone(),
                    lang: lang.take(),
                });
                in_code = false;
            }
            _ => {}
        }
    }
    blocks
}

pub async fn write_text_file(text: &str, filename: &str) -> anyhow::Result<String> {
    let file_path = write_copy_fallback(text, filename).await?;
    let line_count = text.lines().count();
    let char_count = text.chars().count();
    Ok(format!(
        "Écrit : {file_path} ({char_count} caractères, {line_count} lignes)"
    ))
}

/// Copie vers presse-papiers + fichier temporaire de secours.
pub async fn copy_text(text: &str, filename: &str) -> anyhow::Result<String> {
    let line_count = text.lines().count();
    let char_count = text.chars().count();
    let clipboard_ok = copy_to_clipboard(text).is_ok();
    let file_path = write_copy_fallback(text, filename).await?;

    if clipboard_ok {
        Ok(format!(
            "Copié dans le presse-papiers ({char_count} caractères, {line_count} lignes)\nÉgalement écrit : {file_path}"
        ))
    } else {
        Ok(format!(
            "Presse-papiers indisponible — fichier écrit : {file_path} ({char_count} caractères, {line_count} lignes)"
        ))
    }
}

/// Contenu et nom de fichier pour un choix du sélecteur.
#[must_use]
pub fn selection_content(
    full_text: &str,
    blocks: &[CopyCodeBlock],
    choice: &CopyChoice,
) -> (String, String) {
    match choice {
        CopyChoice::FullResponse | CopyChoice::AlwaysFullResponse => {
            (full_text.to_string(), RESPONSE_FILENAME.into())
        }
        CopyChoice::CodeBlock(idx) => {
            let block = &blocks[*idx];
            (
                block.code.clone(),
                format!("copy{}", file_extension(block.lang.as_deref())),
            )
        }
    }
}

#[must_use]
pub fn file_extension(lang: Option<&str>) -> String {
    let Some(lang) = lang else {
        return ".txt".into();
    };
    let sanitized: String = lang
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if sanitized.is_empty() || sanitized.eq_ignore_ascii_case("plaintext") {
        ".txt".into()
    } else {
        format!(".{sanitized}")
    }
}

pub fn try_copy_clipboard(text: &str) -> bool {
    copy_to_clipboard(text).is_ok()
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let mut child = Command::new("cmd")
            .args(["/C", "clip"])
            .stdin(Stdio::piped())
            .spawn()
            .context("lancement clip.exe")?;
        child
            .stdin
            .as_mut()
            .context("stdin clip")?
            .write_all(text.as_bytes())?;
        if !child.wait()?.success() {
            anyhow::bail!("clip.exe a échoué");
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        if try_pipe_clipboard("wl-copy", &[], text).is_ok() {
            return Ok(());
        }
        try_pipe_clipboard("xclip", &["-selection", "clipboard"], text)
    }
}

#[cfg(not(windows))]
fn try_pipe_clipboard(bin: &str, args: &[&str], text: &str) -> anyhow::Result<()> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("lancement {bin}"))?;
    child
        .stdin
        .as_mut()
        .context("stdin clipboard")?
        .write_all(text.as_bytes())?;
    if !child.wait()?.success() {
        anyhow::bail!("{bin} a échoué");
    }
    Ok(())
}

async fn write_copy_fallback(text: &str, filename: &str) -> anyhow::Result<Utf8PathBuf> {
    let dir = std::env::temp_dir().join("drox");
    tokio::fs::create_dir_all(&dir)
        .await
        .context("création répertoire temp drox")?;
    let path = dir.join(filename);
    tokio::fs::write(&path, text)
        .await
        .context("écriture fichier copie")?;
    Utf8PathBuf::from_path_buf(path).map_err(|e| anyhow::anyhow!("chemin temp invalide : {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_assistant_in_reverse_order() {
        let entries = vec![
            LogEntry::User {
                text: "hi".into(),
            },
            LogEntry::Assistant {
                text: "first".into(),
            },
            LogEntry::Assistant {
                text: "second".into(),
            },
        ];
        let texts = collect_recent_assistant_texts(&entries);
        assert_eq!(texts, vec!["second".to_string(), "first".to_string()]);
    }

    #[test]
    fn extracts_fenced_blocks() {
        let md = "text\n\n```rust\nfn main() {}\n```\n";
        let blocks = extract_code_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].lang.as_deref(), Some("rust"));
        assert!(blocks[0].code.contains("fn main"));
    }

    #[test]
    fn extension_sanitizes_lang() {
        assert_eq!(file_extension(Some("python")), ".python");
        assert_eq!(file_extension(Some("../../x")), ".x");
    }
}
