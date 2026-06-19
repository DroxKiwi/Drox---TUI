//! Matching d'un contenu (command, path, …) contre un pattern de règle.
//!
//! Supporte trois variantes de patterns (compatibles avec le format TS) :
//!
//! - **Exact** : pas de wildcard, l'égalité stricte est requise.
//! - **Legacy prefix** : forme `prefix:*`, matche tout ce qui commence par
//!   `prefix` (séparateur espace optionnel après le préfixe).
//! - **Wildcard** : `*` matche `.*` ; `\*` matche un astérisque littéral ;
//!   `\\` matche un backslash littéral. Un seul `*` final précédé d'un espace
//!   rend la fin optionnelle (`git *` matche `git` et `git add`), pour
//!   s'aligner avec la sémantique des prefixes.

use regex::Regex;

/// Pattern parsé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPattern {
    Exact(String),
    Prefix(String),
    Wildcard(String),
}

impl ShellPattern {
    /// Parse un pattern à partir d'un contenu de règle brut.
    #[must_use]
    pub fn parse(rule_content: &str) -> Self {
        if let Some(prefix) = extract_legacy_prefix(rule_content) {
            return Self::Prefix(prefix);
        }
        if has_unescaped_wildcard(rule_content) {
            return Self::Wildcard(rule_content.to_string());
        }
        Self::Exact(rule_content.to_string())
    }

    /// `true` si `command` matche ce pattern (case-sensitive).
    #[must_use]
    pub fn matches(&self, command: &str) -> bool {
        match self {
            Self::Exact(needle) => needle == command,
            Self::Prefix(prefix) => command_starts_with_prefix(command, prefix),
            Self::Wildcard(pattern) => match_wildcard(pattern, command),
        }
    }
}

/// Extrait `prefix` à partir du format legacy `prefix:*`.
fn extract_legacy_prefix(s: &str) -> Option<String> {
    let trimmed = s.trim_end();
    let stripped = trimmed.strip_suffix(":*")?;
    if stripped.is_empty() {
        return None;
    }
    Some(stripped.to_string())
}

/// `true` si la chaîne contient au moins un `*` non échappé qui n'est pas
/// la fin d'un `:*` legacy.
fn has_unescaped_wildcard(s: &str) -> bool {
    if s.ends_with(":*") {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'*' {
            let mut backslashes = 0_usize;
            let mut j = i;
            while j > 0 {
                j -= 1;
                if bytes[j] == b'\\' {
                    backslashes += 1;
                } else {
                    break;
                }
            }
            if backslashes % 2 == 0 {
                return true;
            }
        }
    }
    false
}

/// Vérifie qu'une commande commence par `prefix`, suivi d'une frontière
/// (fin de chaîne ou caractère séparateur). On accepte aussi l'égalité
/// stricte (commande = prefix).
fn command_starts_with_prefix(command: &str, prefix: &str) -> bool {
    if !command.starts_with(prefix) {
        return false;
    }
    let rest = &command[prefix.len()..];
    rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_whitespace())
}

/// Compile un pattern wildcard en regex et l'évalue.
fn match_wildcard(pattern: &str, command: &str) -> bool {
    let Some(regex) = build_wildcard_regex(pattern) else {
        return false;
    };
    regex.is_match(command)
}

/// Convertit un pattern en regex. Retourne `None` si la regex ne compile pas.
fn build_wildcard_regex(pattern: &str) -> Option<Regex> {
    const ESC_STAR: &str = "\u{0}STAR\u{0}";
    const ESC_BSLASH: &str = "\u{0}BSL\u{0}";

    let trimmed = pattern.trim();

    let mut processed = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars().peekable();
    let mut unescaped_stars: usize = 0;
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('*') => {
                    chars.next();
                    processed.push_str(ESC_STAR);
                    continue;
                }
                Some('\\') => {
                    chars.next();
                    processed.push_str(ESC_BSLASH);
                    continue;
                }
                _ => {}
            }
        }
        if c == '*' {
            unescaped_stars += 1;
        }
        processed.push(c);
    }

    let escaped: String = processed
        .chars()
        .flat_map(|c| {
            if matches!(
                c,
                '.' | '+'
                    | '?'
                    | '^'
                    | '$'
                    | '{'
                    | '}'
                    | '('
                    | ')'
                    | '|'
                    | '['
                    | ']'
                    | '\\'
                    | '\''
                    | '"'
            ) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect();

    let with_wildcards = escaped.replace('*', ".*");

    let mut regex_pattern = with_wildcards
        .replace(ESC_STAR, r"\*")
        .replace(ESC_BSLASH, r"\\");

    // `cmd *` (un seul wildcard, en fin, précédé d'un espace) doit aussi
    // matcher `cmd` sans arguments — comme le prefix legacy `cmd:*`.
    if unescaped_stars == 1 && regex_pattern.ends_with(" .*") {
        let len = regex_pattern.len();
        regex_pattern.truncate(len - 3);
        regex_pattern.push_str("( .*)?");
    }

    let final_pattern = format!("(?s)^{regex_pattern}$");
    Regex::new(&final_pattern).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact() {
        assert_eq!(
            ShellPattern::parse("npm install"),
            ShellPattern::Exact("npm install".into())
        );
    }

    #[test]
    fn parses_legacy_prefix() {
        assert_eq!(
            ShellPattern::parse("npm:*"),
            ShellPattern::Prefix("npm".into())
        );
        assert_eq!(
            ShellPattern::parse("git push:*"),
            ShellPattern::Prefix("git push".into())
        );
    }

    #[test]
    fn parses_wildcard() {
        assert_eq!(
            ShellPattern::parse("git *"),
            ShellPattern::Wildcard("git *".into())
        );
    }

    #[test]
    fn exact_match() {
        let p = ShellPattern::parse("npm install");
        assert!(p.matches("npm install"));
        assert!(!p.matches("npm install foo"));
    }

    #[test]
    fn prefix_match() {
        let p = ShellPattern::parse("npm:*");
        assert!(p.matches("npm"));
        assert!(p.matches("npm install"));
        assert!(p.matches("npm run build"));
        assert!(!p.matches("npmx run"));
        assert!(!p.matches("yarn npm"));
    }

    #[test]
    fn wildcard_basic() {
        let p = ShellPattern::parse("git *");
        assert!(p.matches("git status"));
        assert!(p.matches("git push --force"));
        // Avec l'extension trailing-space, `git *` matche aussi `git`.
        assert!(p.matches("git"));
        assert!(!p.matches("gitk"));
        assert!(!p.matches("foo git"));
    }

    #[test]
    fn wildcard_in_middle() {
        let p = ShellPattern::parse("rm -rf *");
        assert!(p.matches("rm -rf /tmp/foo"));
        assert!(!p.matches("rm /tmp"));
    }

    #[test]
    fn wildcard_escaped_star_is_literal_inside_wildcard_pattern() {
        // Pattern : "echo \* *" → matche "echo *" suivi de n'importe quoi,
        // mais pas "echo hello".
        let p = ShellPattern::parse(r"echo \* *");
        assert!(p.matches("echo * dangerously"));
        assert!(p.matches("echo *"));
        assert!(!p.matches("echo hello world"));
    }

    #[test]
    fn wildcard_dotted_command() {
        let p = ShellPattern::parse("python3.* -m *");
        assert!(p.matches("python3.10 -m pip install drox"));
        assert!(!p.matches("python2 -m pip install"));
    }

    #[test]
    fn wildcard_multi_star_does_not_make_trailing_optional() {
        // Avec plusieurs wildcards on ne déclenche pas l'extension trailing-space.
        let p = ShellPattern::parse("* run *");
        assert!(p.matches("npm run build"));
        assert!(!p.matches("npm run"));
    }
}
