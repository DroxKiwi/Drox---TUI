//! Journal de sessions persistant — `.drox/memory/sessions/`.
//!
//! Sprint M1 (mémoire unifiée). Chaque session non triviale laisse une
//! trace markdown dans `<workspace>/.drox/memory/sessions/YYYY-MM-DD-HHMMSS-<slug>.md`
//! (heure UTC dans le nom pour dédoublonner plusieurs runs le même jour).
//! Le contenu est produit par le mécanisme de compaction du moteur
//! ([`drox_engine::compaction`]) éventuellement enrichi par des notes
//! épinglées via le tool `session_note`.
//!
//! Ce module fournit uniquement les helpers d'I/O et de format :
//!
//! - [`slugify`] : normalise un titre libre en slug ASCII court.
//! - [`compute_session_path`] : chemin canonique + dédup de collision.
//! - [`write_session`] / [`read_session`] : I/O markdown.
//! - [`load_sessions_listing`] : lit le front-matter de chaque .md et
//!   renvoie une liste triée du plus récent au plus ancien.
//! - [`format_sessions_listing_for_prompt`] : produit le bloc texte
//!   à injecter dans le system prompt en début de run.
//!
//! Le moteur reste agnostique du format physique : il appelle ces
//! fonctions au moment de persister / recharger.

use std::cmp::Reverse;
use std::fmt::Write as _;

use camino::{Utf8Path, Utf8PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::SessionError;

/// Sous-dossier du workspace où vivent les sessions archivées.
const SESSIONS_SUBDIR: &str = ".drox/memory/sessions";

/// Slug max — assez pour rester lisible, assez court pour rentrer dans
/// le tableau du listing sans casser la mise en page.
const SLUG_MAX_LEN: usize = 50;

/// Limite par défaut pour le listing injecté au system prompt — au-delà,
/// on tue le prompt cache et on noie le modèle. Choisi à 10 entrées
/// (≈ 10 lignes ajoutées) ; réajustable côté appelant.
pub const DEFAULT_LISTING_LIMIT: usize = 10;

/// Front-matter YAML simplifié écrit en tête de chaque session.
///
/// Volontairement plat (pas de structures imbriquées) pour autoriser un
/// parsing maison ligne par ligne sans dépendance `serde_yaml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionFrontMatter {
    /// Slug court ASCII (kebab-case), réutilisé pour le nom du fichier.
    pub slug: String,
    /// Objectif **court** du run (1 ligne), tel qu'extrait par la compaction.
    pub objective: String,
    /// Date de fin de run (UTC, ISO 8601).
    pub date: DateTime<Utc>,
    /// Nom du modèle utilisé (sert au diagnostic ; format libre).
    pub model: String,
    /// Fichiers touchés pendant le run (chemins workspace-relatifs).
    #[serde(default)]
    pub files_touched: Vec<String>,
}

/// Entrée du listing renvoyée par [`load_sessions_listing`].
///
/// Volontairement minimaliste : juste de quoi afficher une ligne par
/// session au modèle, sans charger le body en entier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySessionEntry {
    /// Slug — clé d'identification, passée ensuite à `memory_read`.
    pub slug: String,
    /// Chemin absolu du .md (utile pour l'UI : "ouvrir dans l'éditeur").
    pub path: Utf8PathBuf,
    /// Date de fin de run.
    pub date: DateTime<Utc>,
    /// Objectif (= 1 ligne de résumé).
    pub objective: String,
    /// Fichiers touchés (peut être vide).
    pub files_touched: Vec<String>,
    /// Modèle ayant produit la session.
    pub model: String,
}

/// Résultat de [`search_sessions`] — session archivée pertinente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySearchHit {
    pub slug: String,
    pub path: Utf8PathBuf,
    /// Score heuristique (plus = plus pertinent).
    pub score: u32,
    pub objective: String,
    /// Extrait du corps (1–2 lignes).
    pub snippet: String,
}

/// Normalise un titre libre en slug ASCII court.
///
/// Règles :
/// - minuscules,
/// - lettres ASCII / chiffres conservés, espaces et caractères spéciaux
///   collapsés en `-`,
/// - bordures `-` strippées,
/// - tronqué à [`SLUG_MAX_LEN`] caractères (sans couper en plein milieu
///   d'un groupe de chiffres si possible).
///
/// Renvoie `"session"` si l'input ne contient aucun caractère exploitable.
#[must_use]
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = true;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        let keep = lower.is_ascii_alphanumeric();
        if keep {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    if out.len() > SLUG_MAX_LEN {
        out.truncate(SLUG_MAX_LEN);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        "session".to_string()
    } else {
        out
    }
}

/// Construit le chemin canonique d'une session.
///
/// Format : `<workspace>/.drox/memory/sessions/YYYY-MM-DD-HHMMSS-<slug>.md`
/// (date + heure UTC, sans `:` — compatibilité noms de fichiers Windows).
///
/// **Ne dédoublonne pas** une collision existante — cf. [`reserve_session_path`].
#[must_use]
pub fn compute_session_path(
    workspace: &Utf8Path,
    date: DateTime<Utc>,
    slug: &str,
) -> Utf8PathBuf {
    let stamp = date.format("%Y-%m-%d-%H%M%S").to_string();
    workspace
        .join(SESSIONS_SUBDIR)
        .join(format!("{stamp}-{slug}.md"))
}

/// Comme [`compute_session_path`] mais dédoublonne (`_2`, `_3`, …) si un
/// fichier existe déjà au même chemin.
///
/// Borne dure à 99 collisions — au-delà on retourne `_99` qui écrasera.
/// En pratique, deux runs avec le même slug le même jour, c'est déjà rare ;
/// 99, c'est largement assez.
#[must_use]
pub fn reserve_session_path(
    workspace: &Utf8Path,
    date: DateTime<Utc>,
    slug: &str,
) -> Utf8PathBuf {
    let base = compute_session_path(workspace, date, slug);
    if !base.exists() {
        return base;
    }
    for n in 2..=99 {
        let candidate_slug = format!("{slug}_{n}");
        let p = compute_session_path(workspace, date, &candidate_slug);
        if !p.exists() {
            return p;
        }
    }
    compute_session_path(workspace, date, &format!("{slug}_99"))
}

/// Écrit une session sur disque. Crée le dossier parent si besoin.
///
/// Format du fichier :
///
/// ```text
/// ---
/// slug: <slug>
/// objective: <objective>
/// date: <ISO 8601>
/// model: <model>
/// files_touched:
///   - path/un
///   - path/deux
/// ---
///
/// <body markdown libre>
/// ```
///
/// Le body peut être vide, tronqué, ou tenir plusieurs Ko — c'est laissé
/// à l'appelant. Le format est lu en miroir par [`read_session`].
pub async fn write_session(
    path: &Utf8Path,
    front: &SessionFrontMatter,
    body: &str,
) -> Result<(), SessionError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent.as_std_path()).await?;
    }
    let serialized = render_session(front, body);
    tokio::fs::write(path.as_std_path(), serialized).await?;
    Ok(())
}

/// Lit le contenu **complet** (front-matter + body) d'une session.
///
/// Renvoie la chaîne brute — l'appelant décide de l'afficher tel quel
/// ou de re-parser pour ré-extraire le front-matter.
pub async fn read_session(path: &Utf8Path) -> Result<String, SessionError> {
    let s = tokio::fs::read_to_string(path.as_std_path()).await?;
    Ok(s)
}

/// Liste les sessions présentes dans `<workspace>/.drox/memory/sessions/`,
/// triées par **date décroissante** (plus récent en tête), limitées à
/// `limit` entrées (0 = pas de limite).
///
/// Tolérant aux fichiers mal formés : ignore silencieusement ceux dont le
/// front-matter ne peut pas être parsé. Cette robustesse est volontaire :
/// l'utilisateur peut éditer manuellement les .md et casser le format
/// sans pour autant rendre tout le listing inutilisable.
pub async fn load_sessions_listing(
    workspace: &Utf8Path,
    limit: usize,
) -> Result<Vec<MemorySessionEntry>, SessionError> {
    let dir = workspace.join(SESSIONS_SUBDIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut read_dir = tokio::fs::read_dir(dir.as_std_path()).await?;
    let mut entries: Vec<MemorySessionEntry> = Vec::new();
    while let Some(dirent) = read_dir.next_entry().await? {
        let p = dirent.path();
        if p.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(utf) = Utf8PathBuf::try_from(p.clone()) else {
            continue;
        };
        let Ok(raw) = tokio::fs::read_to_string(&p).await else {
            continue;
        };
        if let Some(front) = parse_front_matter(&raw) {
            entries.push(MemorySessionEntry {
                slug: front.slug,
                path: utf,
                date: front.date,
                objective: front.objective,
                files_touched: front.files_touched,
                model: front.model,
            });
        }
    }
    entries.sort_by_key(|e| Reverse(e.date)); // tri descendant : plus récent en tête
    if limit > 0 && entries.len() > limit {
        entries.truncate(limit);
    }
    Ok(entries)
}

/// Recherche textuelle dans les sessions archivées (slug, objectif, fichiers, corps).
///
/// Alternative locale au tool `session_search` (client IDE). Insensible à la casse ;
/// score par nombre de mots-clés trouvés.
pub async fn search_sessions(
    workspace: &Utf8Path,
    query: &str,
    limit: usize,
) -> Result<Vec<MemorySearchHit>, SessionError> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|w| w.to_ascii_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 30);
    let entries = load_sessions_listing(workspace, 0).await?;
    let mut hits: Vec<MemorySearchHit> = Vec::new();
    for entry in entries {
        let raw = match read_session(&entry.path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let body_lower = raw.to_ascii_lowercase();
        let slug_l = entry.slug.to_ascii_lowercase();
        let obj_l = entry.objective.to_ascii_lowercase();
        let files_l = entry
            .files_touched
            .iter()
            .map(|f| f.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let mut score = 0u32;
        for term in &terms {
            if slug_l.contains(term) {
                score += 12;
            }
            if obj_l.contains(term) {
                score += 8;
            }
            if files_l.contains(term) {
                score += 5;
            }
            if body_lower.contains(term) {
                score += 2;
            }
        }
        if score == 0 {
            continue;
        }
        let snippet = extract_snippet(&raw, &terms).unwrap_or_else(|| entry.objective.clone());
        hits.push(MemorySearchHit {
            slug: entry.slug,
            path: entry.path,
            score,
            objective: entry.objective,
            snippet,
        });
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.slug.cmp(&b.slug)));
    hits.truncate(limit);
    Ok(hits)
}

fn extract_snippet(raw: &str, terms: &[String]) -> Option<String> {
    let body = raw.split("---\n").nth(2).unwrap_or(raw);
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let lower = line.to_ascii_lowercase();
        if terms.iter().any(|t| lower.contains(t)) {
            let s = if line.len() > 120 {
                format!("{}…", &line[..117])
            } else {
                line.to_string()
            };
            return Some(s);
        }
    }
    None
}

/// Convertit un listing en bloc texte prêt à coller dans le system prompt.
///
/// Renvoie `None` si la liste est vide — l'appelant ne doit alors PAS
/// injecter d'en-tête vide.
///
/// Format produit (compact, lisible humain et LLM) :
///
/// ```text
/// [Memory — sessions récentes du workspace (.drox/memory/sessions/)]
/// - 2026-05-13 17:42:00 UTC — refonte-phases : refonte protocole [phase: ...] (engine + prompts)
/// - 2026-05-12 09:15:30 UTC — vision-images : Content::Image + multimodal Ollama
/// ...
/// Use `memory_read { slug: "..." }` to recall the full content of a past session.
/// ```
#[must_use]
pub fn format_sessions_listing_for_prompt(entries: &[MemorySessionEntry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str(
        "[Memory — sessions récentes du workspace (`.drox/memory/sessions/`)]\n",
    );
    for e in entries {
        let when = e
            .date
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string();
        let obj = if e.objective.is_empty() {
            "(no objective)"
        } else {
            e.objective.as_str()
        };
        let _ = writeln!(out, "- {when} — {slug} : {obj}", slug = e.slug, obj = obj);
    }
    out.push_str(
        "Use `memory_read { slug: \"…\" }` to recall the full content of a past session.\n",
    );
    Some(out)
}

// ---------------------------------------------------------------------------
// Helpers internes : sérialisation / parsing du front-matter YAML simplifié.
// ---------------------------------------------------------------------------

fn render_session(front: &SessionFrontMatter, body: &str) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    let _ = writeln!(out, "slug: {}", front.slug);
    let _ = writeln!(out, "objective: {}", escape_single_line(&front.objective));
    let _ = writeln!(
        out,
        "date: {}",
        front.date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    );
    let _ = writeln!(out, "model: {}", escape_single_line(&front.model));
    if front.files_touched.is_empty() {
        out.push_str("files_touched: []\n");
    } else {
        out.push_str("files_touched:\n");
        for p in &front.files_touched {
            let _ = writeln!(out, "  - {}", escape_single_line(p));
        }
    }
    out.push_str("---\n\n");
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Parse le front-matter d'un fichier markdown. Format attendu :
///
/// ```text
/// ---
/// slug: …
/// objective: …
/// date: 2026-05-13T17:42:00Z
/// model: …
/// files_touched:
///   - path/un
///   - path/deux
/// ---
/// ```
///
/// Retourne `None` si le format ne match pas (pas de bornes `---`, ou
/// champs requis absents).
fn parse_front_matter(raw: &str) -> Option<SessionFrontMatter> {
    let mut lines = raw.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut slug = None;
    let mut objective = None;
    let mut date_str = None;
    let mut model = None;
    let mut files_touched: Vec<String> = Vec::new();
    let mut current_list_key: Option<&str> = None;

    for line in lines.by_ref() {
        let trimmed = line.trim_end();
        if trimmed == "---" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("  - ") {
            if current_list_key == Some("files_touched") {
                files_touched.push(rest.trim().to_string());
            }
            continue;
        }
        current_list_key = None;
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "slug" => slug = Some(val.to_string()),
                "objective" => objective = Some(val.to_string()),
                "date" => date_str = Some(val.to_string()),
                "model" => model = Some(val.to_string()),
                "files_touched" => {
                    if val.starts_with('[') && val.ends_with(']') {
                        let inner = &val[1..val.len() - 1];
                        if !inner.trim().is_empty() {
                            for item in inner.split(',') {
                                files_touched.push(item.trim().to_string());
                            }
                        }
                    } else {
                        current_list_key = Some("files_touched");
                    }
                }
                _ => {}
            }
        }
    }

    let date = DateTime::parse_from_rfc3339(date_str.as_deref()?)
        .ok()?
        .with_timezone(&Utc);
    Some(SessionFrontMatter {
        slug: slug?,
        objective: objective?,
        date,
        model: model?,
        files_touched,
    })
}

/// Échappe un payload destiné à tenir sur une ligne YAML simple.
///
/// On reste minimaliste : on retire les retours à la ligne (collapse en
/// espace) et on tronque à 500 chars pour éviter qu'un objectif géant
/// casse le listing. Pas de quoting style YAML (`"…"`) — le parsing
/// `split_once(':')` qu'on utilise en miroir n'en a pas besoin tant qu'on
/// est sur une seule ligne.
fn escape_single_line(s: &str) -> String {
    let mut t: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if t.len() > 500 {
        t.truncate(500);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::tempdir;

    #[test]
    fn slugify_normalizes_titles() {
        assert_eq!(slugify("Refonte Protocole [phase: ...]"), "refonte-protocole-phase");
        assert_eq!(slugify("  Hello  World  "), "hello-world");
        assert_eq!(slugify("été 2026 / éval-modèle"), "t-2026-val-mod-le");
        assert_eq!(slugify(""), "session");
        assert_eq!(slugify("///"), "session");
    }

    #[test]
    fn slugify_truncates_long_inputs_without_trailing_dash() {
        let long = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod";
        let s = slugify(long);
        assert!(s.len() <= SLUG_MAX_LEN);
        assert!(!s.ends_with('-'));
        assert!(s.starts_with("lorem-ipsum"));
    }

    #[test]
    fn compute_session_path_uses_utc_timestamp_in_filename() {
        let ws = Utf8PathBuf::from("/ws");
        let date = Utc.with_ymd_and_hms(2026, 5, 13, 17, 42, 0).unwrap();
        let p = compute_session_path(&ws, date, "refonte-phases");
        // On compare par composants pour rester portable (Windows vs Unix
        // séparateur). On vérifie le nom de fichier et qu'on a bien traversé
        // .drox/memory/sessions/ quelque part dans le chemin.
        assert_eq!(
            p.file_name(),
            Some("2026-05-13-174200-refonte-phases.md")
        );
        let s = p.as_str().replace('\\', "/");
        assert!(s.ends_with(
            "/.drox/memory/sessions/2026-05-13-174200-refonte-phases.md"
        ));
        assert!(s.starts_with("/ws"));
    }

    #[tokio::test]
    async fn write_then_read_round_trip() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let date = Utc.with_ymd_and_hms(2026, 5, 13, 17, 42, 0).unwrap();
        let path = compute_session_path(&ws, date, "test");
        let front = SessionFrontMatter {
            slug: "test".into(),
            objective: "Test round-trip".into(),
            date,
            model: "glm-4.7-flash".into(),
            files_touched: vec!["src/main.rs".into(), "Cargo.toml".into()],
        };
        let body = "## Décisions\n- A\n- B\n";
        write_session(&path, &front, body).await.unwrap();

        let raw = read_session(&path).await.unwrap();
        assert!(raw.contains("slug: test"));
        assert!(raw.contains("model: glm-4.7-flash"));
        assert!(raw.contains("- src/main.rs"));
        assert!(raw.contains("## Décisions"));
    }

    #[tokio::test]
    async fn listing_sorts_by_date_desc_and_tolerates_bad_files() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

        let older = Utc.with_ymd_and_hms(2026, 5, 10, 9, 0, 0).unwrap();
        let newer = Utc.with_ymd_and_hms(2026, 5, 13, 18, 0, 0).unwrap();
        write_session(
            &compute_session_path(&ws, older, "a"),
            &SessionFrontMatter {
                slug: "a".into(),
                objective: "older".into(),
                date: older,
                model: "m".into(),
                files_touched: vec![],
            },
            "",
        )
        .await
        .unwrap();
        write_session(
            &compute_session_path(&ws, newer, "b"),
            &SessionFrontMatter {
                slug: "b".into(),
                objective: "newer".into(),
                date: newer,
                model: "m".into(),
                files_touched: vec![],
            },
            "",
        )
        .await
        .unwrap();
        // Fichier mal formé — doit être ignoré, pas crasher.
        tokio::fs::write(
            ws.join(SESSIONS_SUBDIR)
                .join("2026-05-12-120000-corrupt.md")
                .as_std_path(),
            "no front matter here",
        )
        .await
        .unwrap();

        let entries = load_sessions_listing(&ws, 0).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].slug, "b"); // newer first
        assert_eq!(entries[1].slug, "a");
    }

    #[tokio::test]
    async fn reserve_session_path_dedups_on_collision() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let date = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();

        let p1 = reserve_session_path(&ws, date, "dup");
        write_session(
            &p1,
            &SessionFrontMatter {
                slug: "dup".into(),
                objective: "x".into(),
                date,
                model: "m".into(),
                files_touched: vec![],
            },
            "",
        )
        .await
        .unwrap();

        let p2 = reserve_session_path(&ws, date, "dup");
        assert_ne!(p1, p2);
        assert!(p2
            .as_str()
            .ends_with("2026-05-13-000000-dup_2.md"));
    }

    #[test]
    fn listing_format_is_compact() {
        let date = Utc.with_ymd_and_hms(2026, 5, 13, 18, 0, 0).unwrap();
        let entries = vec![MemorySessionEntry {
            slug: "test".into(),
            path: Utf8PathBuf::from("/ws/.drox/memory/sessions/2026-05-13-180000-test.md"),
            date,
            objective: "Une session test".into(),
            files_touched: vec![],
            model: "glm".into(),
        }];
        let s = format_sessions_listing_for_prompt(&entries).unwrap();
        assert!(s.contains("sessions récentes"));
        assert!(s.contains("2026-05-13 18:00:00 UTC — test : Une session test"));
        assert!(s.contains("memory_read"));
    }

    #[tokio::test]
    async fn search_sessions_finds_keyword_in_body() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let date = Utc.with_ymd_and_hms(2026, 5, 13, 18, 0, 0).unwrap();
        write_session(
            &compute_session_path(&ws, date, "auth-bug"),
            &SessionFrontMatter {
                slug: "auth-bug".into(),
                objective: "corriger login".into(),
                date,
                model: "m".into(),
                files_touched: vec!["src/auth.rs".into()],
            },
            "## Décisions\n- JWT refresh token invalidé côté serveur\n",
        )
        .await
        .unwrap();

        let hits = search_sessions(&ws, "refresh token", 5).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].slug, "auth-bug");
        assert!(hits[0].snippet.contains("refresh"));
    }

    #[test]
    fn listing_format_returns_none_when_empty() {
        assert!(format_sessions_listing_for_prompt(&[]).is_none());
    }
}
