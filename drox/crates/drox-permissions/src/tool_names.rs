//! Noms d'outils pour les règles — alias compatibles leak (`Edit`, `Read`, `Write`).

/// Normalise un nom d'outil issu d'une règle textuelle (settings / CLI).
#[must_use]
pub fn normalize_rule_tool_name(name: &str) -> String {
    match name {
        "Edit" | "FileEdit" => "file_edit".to_string(),
        "Read" | "FileRead" => "file_read".to_string(),
        "Write" | "FileWrite" => "file_write".to_string(),
        other => other.to_string(),
    }
}

/// Nom de règle à consulter pour une cible d'évaluation (notebook/delete → file_edit).
#[must_use]
pub fn primary_rule_tool_name(tool_name: &str) -> &str {
    match tool_name {
        "notebook_edit" | "delete_path" | "copy_path" => "file_edit",
        other => other,
    }
}

/// `true` si le contenu de la règle est un motif de chemin (gitignore), pas une commande shell.
#[must_use]
pub fn uses_path_patterns(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "file_read"
            | "file_write"
            | "file_edit"
            | "notebook_edit"
            | "delete_path"
            | "copy_path"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_leak_aliases() {
        assert_eq!(normalize_rule_tool_name("Edit"), "file_edit");
        assert_eq!(normalize_rule_tool_name("Read"), "file_read");
        assert_eq!(normalize_rule_tool_name("Write"), "file_write");
    }

    #[test]
    fn notebook_edit_uses_file_edit_rules() {
        assert_eq!(primary_rule_tool_name("notebook_edit"), "file_edit");
    }
}
