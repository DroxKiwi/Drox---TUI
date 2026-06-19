//! Correspondance `matcher` → nom d'outil (`bash`, `file_*`, `*`).

use globset::Glob;
use serde_json::Value;

/// Vrai si `matcher` s'applique au nom d'outil.
#[must_use]
pub fn tool_matches(matcher: &str, tool_name: &str) -> bool {
    let m = matcher.trim();
    if m.is_empty() || m == "*" {
        return true;
    }
    if m == tool_name {
        return true;
    }
    Glob::new(m)
        .ok()
        .map(|g| g.compile_matcher())
        .is_some_and(|gm| gm.is_match(tool_name))
}

/// Filtre optionnel `if` (syntaxe simplifiée type leak : `bash`, `file_edit`, `Bash(git *)`).
#[must_use]
pub fn if_condition_matches(condition: &str, tool_name: &str, tool_input: &Value) -> bool {
    let c = condition.trim();
    if c.is_empty() {
        return true;
    }
    if let Some((tool_part, pattern)) = parse_tool_condition(c) {
        if !tool_matches(tool_part, tool_name) {
            return false;
        }
        return input_matches_pattern(tool_name, tool_input, pattern);
    }
    tool_matches(c, tool_name)
}

fn parse_tool_condition(s: &str) -> Option<(&str, &str)> {
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    if close <= open {
        return None;
    }
    Some((s[..open].trim(), s[open + 1..close].trim()))
}

fn input_matches_pattern(tool_name: &str, tool_input: &Value, pattern: &str) -> bool {
    let haystack = if tool_name == "bash" {
        tool_input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    } else {
        tool_input
            .get("path")
            .or_else(|| tool_input.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    if pattern.contains('*') {
        Glob::new(pattern)
            .ok()
            .map(|g| g.compile_matcher())
            .is_some_and(|gm| gm.is_match(haystack))
    } else {
        haystack.contains(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_glob() {
        assert!(tool_matches("bash", "bash"));
        assert!(!tool_matches("bash", "file_read"));
        assert!(tool_matches("file_*", "file_edit"));
        assert!(tool_matches("*", "todo_write"));
    }
}
