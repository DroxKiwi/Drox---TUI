//! Chargement de fichiers d'environnement `KEY=VALUE`.
//!
//! Format supporté, volontairement minimaliste :
//! - Lignes vides ignorées.
//! - Commentaires en `#` (début de ligne ou après un espace).
//! - `KEY=VALUE` — pas de `export`, pas d'expansion `$VAR`.
//! - Valeur entre guillemets simples ou doubles : les guillemets sont retirés.
//! - Une variable déjà définie dans l'environnement n'est **pas** écrasée
//!   (les flags CLI > env shell > `.env` workspace > `.env` user).
//!
//! Ce parser couvre 95 % des cas d'usage sans la lourdeur d'une crate
//! externe `dotenvy`.

use std::path::Path;

/// Charge dans l'ordre (du plus prioritaire au moins prioritaire) :
/// 1. `<workspace>/.drox/.env`   (dotfile, style Unix classique)
/// 2. `<workspace>/.drox/env`    (alias historique sans le point)
/// 3. `~/.drox/.env`
/// 4. `~/.drox/env`
///
/// Une variable déjà définie dans le shell n'est jamais écrasée. Renvoie le
/// nombre de clés effectivement injectées.
pub fn load_default(workspace: Option<&Path>) -> usize {
    let mut applied = 0;
    if let Some(ws) = workspace {
        applied += load_file(ws.join(".drox").join(".env").as_path());
        applied += load_file(ws.join(".drox").join("env").as_path());
    }
    if let Some(home) = dirs::home_dir() {
        applied += load_file(home.join(".drox").join(".env").as_path());
        applied += load_file(home.join(".drox").join("env").as_path());
    }
    applied
}

/// Charge un fichier `.env` au chemin donné. Silencieux si absent.
pub fn load_file(path: &Path) -> usize {
    let Ok(content) = std::fs::read_to_string(path) else {
        return 0;
    };
    let mut applied = 0;
    for line in content.lines() {
        let Some((key, value)) = parse_line(line) else {
            continue;
        };
        if std::env::var_os(&key).is_some() {
            continue;
        }
        // SAFETY (Rust 2024) : `std::env::set_var` est marquée `unsafe` car
        // non thread-safe. On l'invoque ici depuis `main()` **avant** tout
        // démarrage du runtime tokio et **avant** `Cli::parse()`, donc le
        // process est single-threaded à ce moment précis. C'est l'idiom
        // standard pour charger un `.env` au boot.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(&key, &value);
        }
        applied += 1;
    }
    applied
}

/// Découpe une ligne `KEY=VALUE` en `(key, value)`. Renvoie `None` pour les
/// lignes vides, commentaires ou syntaxes invalides.
pub fn parse_line(line: &str) -> Option<(String, String)> {
    let raw = line.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let mut parts = raw.splitn(2, '=');
    let key = parts.next()?.trim();
    let value_raw = parts.next()?.trim();
    if key.is_empty() {
        return None;
    }
    let value = strip_inline_comment_and_quotes(value_raw);
    Some((key.to_string(), value))
}

fn strip_inline_comment_and_quotes(value: &str) -> String {
    let v = value.trim();
    // Si la valeur est entièrement quotée, on retire les quotes et on
    // n'interprète pas un `#` à l'intérieur.
    if v.len() >= 2 {
        let bytes = v.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return v[1..v.len() - 1].to_string();
        }
    }
    // Pas quoté : on coupe tout commentaire inline `# …`.
    let cut = v.find(" #").map_or(v.len(), |i| i);
    v[..cut].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_key_value() {
        assert_eq!(
            parse_line("DROX_API_KEY=sk-abc"),
            Some(("DROX_API_KEY".into(), "sk-abc".into()))
        );
    }

    #[test]
    fn ignores_blank_and_comment_lines() {
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("   "), None);
        assert_eq!(parse_line("# commentaire"), None);
    }

    #[test]
    fn strips_inline_comment_when_unquoted() {
        assert_eq!(
            parse_line("DROX_SERVER=http://x:11434 # local"),
            Some(("DROX_SERVER".into(), "http://x:11434".into()))
        );
    }

    #[test]
    fn keeps_hash_inside_double_quoted_value() {
        assert_eq!(
            parse_line(r#"DROX_API_KEY="sk-#with-hash""#),
            Some(("DROX_API_KEY".into(), "sk-#with-hash".into()))
        );
    }

    #[test]
    fn strips_single_quotes() {
        assert_eq!(
            parse_line("DROX_MODEL='granite4.1:8b'"),
            Some(("DROX_MODEL".into(), "granite4.1:8b".into()))
        );
    }

    #[test]
    fn rejects_lines_without_equals() {
        assert_eq!(parse_line("not a kv pair"), None);
    }

    #[test]
    fn rejects_empty_key() {
        assert_eq!(parse_line("=value"), None);
    }
}
