//! Fichiers `MEMORY.md` / `DROX.md` à la racine du workspace (memdir).

use camino::Utf8Path;

use crate::error::SessionError;

const MEMORY: &str = "MEMORY.md";
const DROX: &str = "DROX.md";

/// Contenu optionnel des deux fichiers memdir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemdirFiles {
    pub memory_md: Option<String>,
    pub drox_md: Option<String>,
}

/// Lit `MEMORY.md` et `DROX.md` s'ils existent sous `workspace`.
pub async fn load_memdir(workspace: &Utf8Path) -> Result<MemdirFiles, SessionError> {
    let memory_path = workspace.join(MEMORY);
    let drox_path = workspace.join(DROX);

    let memory_md = match tokio::fs::read_to_string(memory_path.as_std_path()).await {
        Ok(s) if !s.trim().is_empty() => Some(s),
        Ok(_) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };

    let drox_md = match tokio::fs::read_to_string(drox_path.as_std_path()).await {
        Ok(s) if !s.trim().is_empty() => Some(s),
        Ok(_) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };

    Ok(MemdirFiles { memory_md, drox_md })
}

/// Préfixe texte à injecter dans le system prompt (si non vide).
#[must_use]
pub fn memdir_system_prefix(files: &MemdirFiles) -> Option<String> {
    match (&files.memory_md, &files.drox_md) {
        (None, None) => None,
        (Some(m), None) => Some(format!(
            "The following project memory file is available:\n\n--- MEMORY.md ---\n{m}\n"
        )),
        (None, Some(d)) => Some(format!(
            "The following project instructions file is available:\n\n--- DROX.md ---\n{d}\n"
        )),
        (Some(m), Some(d)) => Some(format!(
            "Project context files:\n\n--- MEMORY.md ---\n{m}\n\n--- DROX.md ---\n{d}\n"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn loads_existing_memdir() {
        let dir = tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        tokio::fs::write(root.join(MEMORY).as_std_path(), "# mem\nhello")
            .await
            .unwrap();
        let m = load_memdir(root).await.unwrap();
        assert!(m.memory_md.as_ref().is_some_and(|s| s.contains("hello")));
        assert!(m.drox_md.is_none());
        let p = memdir_system_prefix(&m).unwrap();
        assert!(p.contains("MEMORY.md"));
    }
}
