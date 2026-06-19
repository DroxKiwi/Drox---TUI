//! Smart paste — gros collages remplacés par des références (leak : `inputPaste.ts`, `history.ts`).
//!
//! Le composer affiche `[Pasted text #N +X lines]` ou `[Image #N]` ; le contenu intégral
//! est réinjecté dans le prompt agent à l'envoi.

use std::collections::HashMap;

use drox_types::Content;

use crate::engine::image_paste::{parse_image_refs, StoredImage};

/// Au-delà de ce seuil, un collage devient une référence (pas le texte brut).
pub const PASTE_INLINE_THRESHOLD: usize = 800;

/// Nombre max de lignes affichées inline dans le composer (hauteur TUI).
pub const PASTE_INLINE_MAX_LINES: usize = 2;

/// Si le message final dépasse ce seuil, tronquer le milieu avec `[...Truncated text #N ...]`.
pub const PASTE_TRUNCATION_THRESHOLD: usize = 10_000;

const PREVIEW_HALF: usize = 500;

/// Contenu texte et images associés à des identifiants de collage (ids partagés).
#[derive(Debug, Clone)]
pub struct PastedTextStore {
    next_id: u32,
    text_entries: HashMap<u32, String>,
    image_entries: HashMap<u32, StoredImage>,
}

impl Default for PastedTextStore {
    fn default() -> Self {
        Self {
            next_id: 1,
            text_entries: HashMap::new(),
            image_entries: HashMap::new(),
        }
    }
}

/// Résultat de préparation du prompt agent (expansion paste + `@` + images).
#[derive(Debug, Clone)]
pub struct PreparedUserPrompt {
    pub user_blocks: Vec<Content>,
    pub notes: Vec<String>,
}

impl PastedTextStore {
    /// Traite un collage presse-papiers : retourne le fragment à insérer dans le composer.
    #[must_use]
    pub fn ingest_paste(&mut self, text: &str) -> String {
        let lines = count_newlines(text);
        if text.len() <= PASTE_INLINE_THRESHOLD && lines <= PASTE_INLINE_MAX_LINES {
            return text.to_string();
        }

        let id = self.next_id;
        self.next_id += 1;
        self.text_entries.insert(id, text.to_string());
        format_pasted_text_ref(id, lines)
    }

    /// Stocke une image et retourne la référence `[Image #N]`.
    #[must_use]
    pub fn ingest_image(&mut self, image: StoredImage) -> String {
        let id = self.next_id;
        self.next_id += 1;
        self.image_entries.insert(id, image);
        crate::engine::image_paste::format_image_ref(id)
    }

    /// Remplace les références `[Pasted text #N]` / `[...Truncated text #N ...]` par le contenu stocké.
    #[must_use]
    pub fn expand_text(&self, input: &str) -> String {
        expand_pasted_text_refs(input, &self.text_entries)
    }

    /// Si le texte dépasse le plafond, extrait le milieu dans le store et retourne une version tronquée.
    #[must_use]
    pub fn truncate_oversized(&mut self, input: &str) -> String {
        if input.len() <= PASTE_TRUNCATION_THRESHOLD || contains_paste_ref(input) {
            return input.to_string();
        }

        let id = self.next_id;
        self.next_id += 1;
        let start_len = PREVIEW_HALF;
        let end_len = PREVIEW_HALF;
        let start_text = &input[..start_len];
        let end_text = &input[input.len() - end_len..];
        let middle = &input[start_len..input.len() - end_len];
        let truncated_lines = count_newlines(middle);

        self.text_entries.insert(id, middle.to_string());
        format!(
            "{start_text}{}{end_text}",
            format_truncated_text_ref(id, truncated_lines)
        )
    }

    pub fn clear(&mut self) {
        self.text_entries.clear();
        self.image_entries.clear();
        self.next_id = 1;
    }
}

/// Prépare les blocs envoyés à l'agent : expansion paste, `@fichier`, puis images référencées.
pub fn prepare_user_prompt(
    store: &mut PastedTextStore,
    display: &str,
    roots: &[camino::Utf8PathBuf],
    ignore: &drox_session::DroxIgnoreMatcher,
) -> PreparedUserPrompt {
    let mut text = store.expand_text(display);
    text = store.truncate_oversized(&text);
    let at = crate::engine::at_refs::expand_at_refs(&text, roots, ignore);
    let mut notes = at.notes;
    let mut blocks = vec![Content::text(at.agent_prompt)];

    // N'envoyer que les images dont le placeholder est encore dans le composer.
    for id in parse_image_refs(display) {
        let Some(img) = store.image_entries.get(&id) else {
            continue;
        };
        blocks.push(Content::image(img.mime.clone(), img.data_base64.clone()));
        notes.push(format!(
            "Image #{id} annexée — {} ({})",
            img.label,
            format_bytes(img.byte_len)
        ));
    }

    PreparedUserPrompt {
        user_blocks: blocks,
        notes,
    }
}

fn format_bytes(n: usize) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} Mo", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.0} Ko", n as f64 / 1024.0)
    } else {
        format!("{n} o")
    }
}

/// Référence affichée dans le composer (`history.ts` : `formatPastedTextRef`).
#[must_use]
pub fn format_pasted_text_ref(id: u32, num_lines: usize) -> String {
    if num_lines == 0 {
        format!("[Pasted text #{id}]")
    } else {
        format!("[Pasted text #{id} +{num_lines} lines]")
    }
}

#[must_use]
fn format_truncated_text_ref(id: u32, num_lines: usize) -> String {
    format!("[...Truncated text #{id} +{num_lines} lines...]")
}

/// Nombre de sauts de ligne (aligné leak : `\n` dans le texte = +N lignes, pas N+1 lignes totales).
#[must_use]
pub fn count_newlines(text: &str) -> usize {
    text.matches('\n').count()
}

/// Parse les références paste dans un message.
#[must_use]
pub fn parse_paste_references(input: &str) -> Vec<PasteReference> {
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = input[search_from..].find('[') {
        let idx = search_from + rel;
        let rest = &input[idx..];
        if let Some(parsed) = try_parse_ref_at(rest, idx) {
            search_from = parsed.end_index;
            out.push(parsed);
        } else {
            search_from = idx + 1;
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteReference {
    pub id: u32,
    pub matched: String,
    pub start_index: usize,
    pub end_index: usize,
}

fn try_parse_ref_at(rest: &str, start_index: usize) -> Option<PasteReference> {
    // [Pasted text #1] ou [Pasted text #1 +10 lines]
    if let Some(m) = parse_bracket_ref(rest, "Pasted text") {
        let len = m.matched.len();
        return Some(PasteReference {
            id: m.id,
            matched: m.matched,
            start_index,
            end_index: start_index + len,
        });
    }
    // [...Truncated text #1 +10 lines...]
    if let Some(m) = parse_bracket_ref(rest, "...Truncated text") {
        let len = m.matched.len();
        return Some(PasteReference {
            id: m.id,
            matched: m.matched,
            start_index,
            end_index: start_index + len,
        });
    }
    None
}

struct BracketMatch {
    id: u32,
    matched: String,
}

fn parse_bracket_ref(rest: &str, kind: &str) -> Option<BracketMatch> {
    let prefix = format!("[{kind} #");
    if !rest.starts_with(&prefix) {
        return None;
    }
    let after = &rest[prefix.len()..];
    let id_end = after.find(|c: char| !c.is_ascii_digit())?;
    let id: u32 = after[..id_end].parse().ok()?;
    let close = rest.find(']')?;
    let matched = rest[..=close].to_string();
    Some(BracketMatch { id, matched })
}

/// Remplace les placeholders par le contenu stocké (ordre inverse pour préserver les offsets).
#[must_use]
pub fn expand_pasted_text_refs(input: &str, entries: &HashMap<u32, String>) -> String {
    let refs = parse_paste_references(input);
    if refs.is_empty() {
        return input.to_string();
    }
    let mut expanded = input.to_string();
    for r in refs.iter().rev() {
        let Some(content) = entries.get(&r.id) else {
            continue;
        };
        expanded.replace_range(r.start_index..r.end_index, content);
    }
    expanded
}

fn contains_paste_ref(input: &str) -> bool {
    input.contains("[Pasted text #") || input.contains("[...Truncated text #")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_large_paste_returns_ref() {
        let mut store = PastedTextStore::default();
        let big = "x\n".repeat(100);
        let ins = store.ingest_paste(&big);
        assert!(ins.contains("[Pasted text #1"));
        assert_eq!(store.text_entries.get(&1).map(String::len), Some(big.len()));
    }

    #[test]
    fn small_paste_stays_inline() {
        let mut store = PastedTextStore::default();
        let ins = store.ingest_paste("hello");
        assert_eq!(ins, "hello");
        assert!(store.text_entries.is_empty());
    }

    #[test]
    fn expand_restores_content() {
        let mut store = PastedTextStore::default();
        let body = "line1\nline2\nline3";
        store.text_entries.insert(2, body.to_string());
        let input = "voir [Pasted text #2 +2 lines] merci";
        assert_eq!(store.expand_text(input), "voir line1\nline2\nline3 merci");
    }

    #[test]
    fn truncate_oversized_middle() {
        let mut store = PastedTextStore::default();
        let huge = "a".repeat(12_000);
        let truncated = store.truncate_oversized(&huge);
        assert!(truncated.len() < huge.len());
        assert!(truncated.contains("[...Truncated text #"));
        let expanded = store.expand_text(&truncated);
        assert_eq!(expanded.len(), huge.len());
    }

    #[test]
    fn prepare_includes_referenced_images() {
        let mut store = PastedTextStore::default();
        let ins = store.ingest_image(StoredImage {
            mime: "image/png".into(),
            data_base64: "abc".into(),
            label: "test.png".into(),
            byte_len: 3,
        });
        assert_eq!(ins, "[Image #1]");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let root = camino::Utf8PathBuf::from(
            std::env::temp_dir()
                .join("drox_tui_paste_test")
                .to_string_lossy()
                .into_owned(),
        );
        let _ = std::fs::create_dir_all(root.as_std_path());
        let ignore = rt
            .block_on(drox_session::DroxIgnoreMatcher::load_or_create(root))
            .unwrap();
        let prepared = prepare_user_prompt(&mut store, "décris [Image #1]", &[], &ignore);
        assert_eq!(prepared.user_blocks.len(), 2);
        assert!(matches!(&prepared.user_blocks[1], Content::Image { .. }));
    }
}
