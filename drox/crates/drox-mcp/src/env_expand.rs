//! Expansion `${VAR}` dans les chaînes (aligné sur le moteur TypeScript).

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::McpError;

static ENV_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("ENV_VAR_RE is valid"));

/// Remplace chaque occurrence de `${NAME}` par `std::env::var("NAME")`.
pub fn expand_env_in_string(input: &str) -> Result<String, McpError> {
    let re = &*ENV_VAR_RE;
    let mut out = String::with_capacity(input.len());
    let mut last = 0usize;
    for cap in re.captures_iter(input) {
        let m = cap.get(0).expect("match 0");
        out.push_str(&input[last..m.start()]);
        let name = cap.get(1).expect("group 1").as_str();
        let value = std::env::var(name).map_err(|_| McpError::MissingEnvVar(name.to_string()))?;
        out.push_str(&value);
        last = m.end();
    }
    out.push_str(&input[last..]);
    Ok(out)
}

/// Applique `expand_env_in_string` à toutes les valeurs d'une table.
#[allow(clippy::implicit_hasher)]
pub fn expand_env_in_map(map: &mut HashMap<String, String>) -> Result<(), McpError> {
    for v in map.values_mut() {
        *v = expand_env_in_string(v)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_no_placeholder_unchanged() {
        let s = expand_env_in_string("plain").unwrap();
        assert_eq!(s, "plain");
    }

    #[test]
    fn missing_var_errors() {
        let err = expand_env_in_string("${THIS_VAR_SHOULD_NOT_EXIST_99999}").unwrap_err();
        assert!(matches!(err, McpError::MissingEnvVar(_)));
    }
}
