//! Internationalisation TUI (FR / EN).

mod catalog_en;
mod catalog_fr;
pub mod keys;
pub mod keys_p1;
mod locale;

use std::sync::RwLock;

pub use keys::{ONBOARDING_STEPS, P0_KEYS};
pub use keys_p1::{COMPOSER_HELP_LINES, P1_KEYS};
pub use locale::UiLocale;

static LOCALE: RwLock<UiLocale> = RwLock::new(UiLocale::Fr);

/// Définit la locale active (re-render immédiat au prochain frame).
pub fn set_locale(locale: UiLocale) {
    if let Ok(mut guard) = LOCALE.write() {
        *guard = locale;
    }
}

/// Locale courante.
#[must_use]
pub fn current_locale() -> UiLocale {
    LOCALE.read().map(|g| *g).unwrap_or(UiLocale::En)
}

/// Traduit une clé stable ; repli EN puis clé brute.
#[must_use]
pub fn t(key: &str) -> &'static str {
    translate(current_locale(), key)
        .or_else(|| translate(UiLocale::En, key))
        .unwrap_or("???")
}

/// Remplace le premier `{}` par `arg`.
#[must_use]
pub fn tf(key: &str, arg: &str) -> String {
    t(key).replacen("{}", arg, 1)
}

/// Remplace deux placeholders `{}` dans l'ordre.
#[must_use]
pub fn tf2(key: &str, a: &str, b: &str) -> String {
    let once = t(key).replacen("{}", a, 1);
    once.replacen("{}", b, 1)
}

#[must_use]
fn translate(locale: UiLocale, key: &str) -> Option<&'static str> {
    match locale {
        UiLocale::Fr => catalog_fr::get(key),
        UiLocale::En => catalog_en::get(key),
    }
}

/// Statut `/server` en erreur (préfixe localisé).
#[must_use]
pub fn server_connection_failed(status: &str) -> bool {
    status.starts_with(t(keys::MODAL_SERVER_CONN_FAILED_PREFIX))
        || status.starts_with(translate(UiLocale::Fr, keys::MODAL_SERVER_CONN_FAILED_PREFIX).unwrap_or(""))
        || status.starts_with(translate(UiLocale::En, keys::MODAL_SERVER_CONN_FAILED_PREFIX).unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_keys_exist_in_fr_and_en() {
        for key in P0_KEYS {
            assert!(
                catalog_fr::get(key).is_some(),
                "clé FR manquante: {key}"
            );
            assert!(
                catalog_en::get(key).is_some(),
                "clé EN manquante: {key}"
            );
        }
    }

    #[test]
    fn p1_keys_exist_in_fr_and_en() {
        for key in P1_KEYS {
            assert!(
                catalog_fr::get(key).is_some(),
                "clé FR manquante: {key}"
            );
            assert!(
                catalog_en::get(key).is_some(),
                "clé EN manquante: {key}"
            );
        }
    }

    #[test]
    fn tf_replaces_placeholder() {
        set_locale(UiLocale::En);
        let s = tf(keys::MODAL_PERMISSION_QUEUE, "2");
        assert!(s.contains('2'));
        assert!(!s.contains("{}"));
    }

    #[test]
    fn locale_parse() {
        assert_eq!(UiLocale::parse("fr-FR"), Some(UiLocale::Fr));
        assert_eq!(UiLocale::parse("en_US"), Some(UiLocale::En));
        assert_eq!(UiLocale::parse("de"), None);
    }
}
