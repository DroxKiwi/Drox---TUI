//! Ollama context window presets (`num_ctx`) and agent limits.

/// Presets shown in `/server` (last entry = custom input).
pub const CONTEXT_PRESETS: &[(&str, i64)] = &[
    ("16k", 16_384),
    ("32k", 32_768),
    ("64k", 65_536),
    ("128k", 131_072),
    ("256k", 262_144),
    ("512k", 524_288),
    ("1M", 1_048_576),
];

pub const CONTEXT_CUSTOM_INDEX: usize = CONTEXT_PRESETS.len();

#[must_use]
pub fn default_num_ctx() -> i64 {
    32_768
}

#[must_use]
pub fn default_max_iterations() -> usize {
    25
}

#[must_use]
pub fn preset_count() -> usize {
    CONTEXT_PRESETS.len() + 1
}

#[must_use]
pub fn cycle_preset_index(current: usize, reverse: bool) -> usize {
    let n = preset_count();
    if reverse {
        (current + n - 1) % n
    } else {
        (current + 1) % n
    }
}

#[must_use]
pub fn preset_index_for(num_ctx: i64) -> usize {
    CONTEXT_PRESETS
        .iter()
        .position(|(_, v)| *v == num_ctx)
        .unwrap_or(CONTEXT_CUSTOM_INDEX)
}

#[must_use]
pub fn preset_label(index: usize, custom_raw: &str) -> String {
    if index < CONTEXT_PRESETS.len() {
        CONTEXT_PRESETS[index].0.to_string()
    } else {
        let t = custom_raw.trim();
        if t.is_empty() {
            "Custom".into()
        } else {
            format!("Custom ({t})")
        }
    }
}

pub fn resolve_num_ctx(preset_index: usize, custom_raw: &str) -> Result<i64, String> {
    if preset_index < CONTEXT_PRESETS.len() {
        return Ok(CONTEXT_PRESETS[preset_index].1);
    }
    let trimmed = custom_raw.trim();
    if trimmed.is_empty() {
        return Err("Custom context: enter a token count".into());
    }
    let lower = trimmed.to_ascii_lowercase().replace('_', "");
    let value = if let Some(stripped) = lower.strip_suffix('k') {
        stripped
            .parse::<i64>()
            .map_err(|_| "Invalid number".to_string())?
            .saturating_mul(1_024)
    } else if let Some(stripped) = lower.strip_suffix('m') {
        stripped
            .parse::<i64>()
            .map_err(|_| "Invalid number".to_string())?
            .saturating_mul(1_024 * 1_024)
    } else {
        trimmed
            .parse::<i64>()
            .map_err(|_| "Invalid number (e.g. 65536, 64k, 128k)".to_string())?
    };
    if value < 2048 {
        return Err("num_ctx minimum: 2048".into());
    }
    Ok(value)
}

pub fn parse_max_iterations(raw: &str) -> Result<usize, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Max iterations: enter a number".into());
    }
    let value: usize = trimmed
        .parse()
        .map_err(|_| "Max iterations: invalid number".to_string())?;
    if value < 1 {
        return Err("Max iterations: minimum 1".into());
    }
    if value > 256 {
        return Err("Max iterations: maximum 256".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_preset_values() {
        assert_eq!(resolve_num_ctx(0, "").unwrap(), 16_384);
        assert_eq!(resolve_num_ctx(2, "").unwrap(), 65_536);
    }

    #[test]
    fn resolves_custom_k_suffix() {
        assert_eq!(resolve_num_ctx(CONTEXT_CUSTOM_INDEX, "128k").unwrap(), 131_072);
    }

    #[test]
    fn maps_saved_value_to_preset() {
        assert_eq!(preset_index_for(32_768), 1);
        assert_eq!(preset_index_for(99_999), CONTEXT_CUSTOM_INDEX);
    }

    #[test]
    fn cycles_presets() {
        assert_eq!(cycle_preset_index(0, false), 1);
        assert_eq!(cycle_preset_index(0, true), CONTEXT_CUSTOM_INDEX);
    }

    #[test]
    fn parses_max_iterations() {
        assert_eq!(parse_max_iterations("25").unwrap(), 25);
        assert!(parse_max_iterations("0").is_err());
        assert!(parse_max_iterations("999").is_err());
    }
}