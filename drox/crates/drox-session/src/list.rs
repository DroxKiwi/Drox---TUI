//! Liste des sessions sur disque (fichiers `*.jsonl`).

use camino::Utf8PathBuf;
use drox_types::SessionId;

use crate::error::SessionError;

/// Métadonnées légères pour affichage / CLI (`drox --list-sessions`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListEntry {
    pub id: SessionId,
    pub path: Utf8PathBuf,
    /// `mtime` du fichier en secondes depuis l'UNIX epoch (best-effort).
    pub modified_secs: u64,
    pub size_bytes: u64,
}

/// Liste les fichiers `ses_*.jsonl` dans `sessions_dir`, triés du plus récent au plus ancien.
pub async fn list_sessions(
    sessions_dir: &camino::Utf8Path,
) -> Result<Vec<SessionListEntry>, SessionError> {
    let mut read = match tokio::fs::read_dir(sessions_dir.as_std_path()).await {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut out = Vec::new();
    while let Some(ent) = read.next_entry().await? {
        let path = Utf8PathBuf::from_path_buf(ent.path()).map_err(|_| SessionError::InvalidPath)?;
        let Some(name) = path.file_name() else {
            continue;
        };
        if !name.starts_with("ses_") {
            continue;
        }
        let ext_ok = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"));
        if !ext_ok {
            continue;
        }
        let Some(stem) = path.file_stem() else {
            continue;
        };
        let id = SessionId::from_string(stem.into());
        let meta = tokio::fs::metadata(&path).await?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let size = meta.len();
        out.push(SessionListEntry {
            id,
            path,
            modified_secs: modified,
            size_bytes: size,
        });
    }

    out.sort_by_key(|e| std::cmp::Reverse(e.modified_secs));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn lists_jsonl_sessions() {
        let dir = tempdir().unwrap();
        let udir = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        tokio::fs::create_dir_all(&udir).await.unwrap();
        let id = SessionId::new();
        let p = crate::paths::transcript_path(&udir, &id);
        tokio::fs::write(&p, b"").await.unwrap();

        let list = list_sessions(&udir).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }
}
