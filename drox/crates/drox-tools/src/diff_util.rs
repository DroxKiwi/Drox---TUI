//! Diff unifié court pour previews permission / propose.

use similar::TextDiff;

/// Diff unifié (`context_radius = 3`) entre deux contenus texte.
#[must_use]
pub fn unified_line_diff(path_label: &str, before: &str, after: &str) -> String {
    let diff = TextDiff::from_lines(before, after);
    diff.unified_diff()
        .context_radius(3)
        .header(path_label, path_label)
        .to_string()
}
