//! Métadonnées auxiliaires session (`ses_*.meta.json`).

use crate::error::SessionError;

/// Titre personnalisé et métadonnées légères (hors transcript JSONL).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
}

/// Lit le fichier meta s'il existe.
pub async fn read_session_meta(path: &camino::Utf8Path) -> Option<SessionMeta> {
    let bytes = tokio::fs::read(path.as_std_path()).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Écriture atomique du fichier meta.
pub async fn write_session_meta(
    path: &camino::Utf8Path,
    meta: &SessionMeta,
) -> Result<(), SessionError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent.as_std_path()).await?;
    }
    let tmp = camino::Utf8PathBuf::from(format!("{path}.tmp"));
    let data = serde_json::to_vec(meta)?;
    tokio::fs::write(tmp.as_std_path(), &data).await?;
    tokio::fs::rename(tmp.as_std_path(), path.as_std_path()).await?;
    Ok(())
}

/// Titre affichable : custom ou identifiant session.
#[must_use]
pub fn display_title(session_id: &str, meta: Option<&SessionMeta>) -> String {
    meta.and_then(|m| m.custom_title.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| session_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn roundtrip_meta() {
        let dir = tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(dir.path().join("ses_x.meta.json")).unwrap();
        let meta = SessionMeta {
            custom_title: Some("Mon run".into()),
        };
        write_session_meta(&p, &meta).await.unwrap();
        let got = read_session_meta(&p).await.unwrap();
        assert_eq!(got.custom_title.as_deref(), Some("Mon run"));
    }

    #[test]
    fn display_title_prefers_custom() {
        let meta = SessionMeta {
            custom_title: Some("Titre".into()),
        };
        assert_eq!(display_title("ses_abc", Some(&meta)), "Titre");
    }
}
