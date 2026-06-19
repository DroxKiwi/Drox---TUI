//! Résolution de la **langue principale** d'interaction avec l'assistant.
//!
//! Lue depuis la variable d'environnement [`DROX_PRIMARY_LANGUAGE_ENV`]
//! (chargée par `.drox/.env` au boot, cf. [`crate::env_file`]). On accepte :
//! - un code court : `fr`, `en`, `es`, `de`, `it`, `pt`, `nl`, `ja`, `zh`, …
//! - un code régional : `fr-CA`, `en-US`, `pt-BR` — seule la partie avant `-`
//!   est utilisée pour identifier la langue de référence ;
//! - un nom de langue libre : `Français`, `English`, `Deutsch`, …
//!
//! La résolution renvoie un [`Language`] avec un **nom affiché** (français,
//! used dans le prompt) et une **instruction système** prête à concaténer
//! aux autres morceaux (memdir / `--system`).

/// Nom de la variable d'environnement.
pub const DROX_PRIMARY_LANGUAGE_ENV: &str = "DROX_PRIMARY_LANGUAGE";

/// Langue résolue avec son nom et son instruction système.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// Nom de la langue dans la langue elle-même (ex. `français`, `English`).
    pub display: String,
    /// Petite instruction à injecter en tête du `system_prompt`.
    pub system_instruction: String,
}

impl Language {
    fn known(code: &str, display: &str) -> Self {
        let system_instruction = format!(
            "Répondez systématiquement en {display} (sauf si l'utilisateur \
             change explicitement de langue ou demande du code dans un \
             autre langage de programmation). Langue principale : {code}."
        );
        Self {
            display: display.into(),
            system_instruction,
        }
    }
}

/// Lit `DROX_PRIMARY_LANGUAGE` depuis l'environnement et la résout. Renvoie
/// `None` si la variable est absente / vide.
#[must_use]
pub fn from_env() -> Option<Language> {
    let raw = std::env::var(DROX_PRIMARY_LANGUAGE_ENV).ok()?;
    parse(&raw)
}

/// Parse une valeur libre (`fr`, `fr-CA`, `Français`, `english`, …). Renvoie
/// `None` si la chaîne est vide après trim.
#[must_use]
pub fn parse(raw: &str) -> Option<Language> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // On ne garde que la partie principale (`fr-CA` → `fr`) pour le mapping.
    let primary = trimmed
        .split(['-', '_', ' '])
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();

    let lang = match primary.as_str() {
        "fr" | "fra" | "french" | "français" | "francais" => {
            Language::known("fr", "français")
        }
        "en" | "eng" | "english" => Language::known("en", "English"),
        "es" | "spa" | "spanish" | "español" | "espanol" => {
            Language::known("es", "español")
        }
        "de" | "ger" | "deu" | "german" | "deutsch" => Language::known("de", "Deutsch"),
        "it" | "ita" | "italian" | "italiano" => Language::known("it", "italiano"),
        "pt" | "por" | "portuguese" | "português" | "portugues" => {
            Language::known("pt", "português")
        }
        "nl" | "nld" | "dutch" | "nederlands" => Language::known("nl", "Nederlands"),
        "ja" | "jpn" | "japanese" | "日本語" => Language::known("ja", "日本語"),
        "zh" | "chi" | "zho" | "chinese" | "中文" => Language::known("zh", "中文"),
        "ko" | "kor" | "korean" | "한국어" => Language::known("ko", "한국어"),
        "ru" | "rus" | "russian" | "русский" => Language::known("ru", "русский"),
        "ar" | "ara" | "arabic" | "العربية" => Language::known("ar", "العربية"),
        "pl" | "pol" | "polish" | "polski" => Language::known("pl", "polski"),
        "tr" | "tur" | "turkish" | "türkçe" | "turkce" => {
            Language::known("tr", "Türkçe")
        }
        other if !other.is_empty() => {
            // Valeur inconnue : on délègue au modèle en lui passant la
            // chaîne brute comme nom de langue.
            Language::known(other, trimmed)
        }
        _ => return None,
    };
    Some(lang)
}

/// Fusionne une instruction de langue avec un éventuel `system_prompt` déjà
/// construit (memdir + `--system` + …). L'instruction de langue est insérée
/// en **tête** pour qu'elle soit le premier signal donné au modèle.
#[must_use]
pub fn merge_into_system(base: Option<String>, lang: Option<&Language>) -> Option<String> {
    match (base, lang) {
        (None, None) => None,
        (Some(s), None) => Some(s),
        (None, Some(l)) => Some(l.system_instruction.clone()),
        (Some(s), Some(l)) => Some(format!("{}\n\n{}", l.system_instruction, s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_short_code() {
        let l = parse("fr").unwrap();
        assert_eq!(l.display, "français");
        assert!(l.system_instruction.contains("français"));
    }

    #[test]
    fn parses_regional_variant() {
        let l = parse("fr-CA").unwrap();
        assert_eq!(l.display, "français");
    }

    #[test]
    fn parses_full_name_case_insensitive() {
        let l = parse("English").unwrap();
        assert_eq!(l.display, "English");
    }

    #[test]
    fn unknown_value_passes_through() {
        let l = parse("Esperanto").unwrap();
        assert_eq!(l.display, "Esperanto");
        assert!(l.system_instruction.contains("Esperanto"));
    }

    #[test]
    fn empty_returns_none() {
        assert!(parse("").is_none());
        assert!(parse("   ").is_none());
    }

    #[test]
    fn merge_into_system_keeps_order() {
        let lang = parse("fr").unwrap();
        let merged = merge_into_system(Some("Mem rules".into()), Some(&lang)).unwrap();
        assert!(merged.starts_with("Répondez"));
        assert!(merged.contains("Mem rules"));
    }
}
