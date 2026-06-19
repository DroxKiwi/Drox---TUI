//! Compteurs d'usage affichés dans la webview (↑ / ↓ / ctx), persistés pour
//! chaque session afin de les restaurer après `session.read`.

use camino::Utf8Path;

use crate::error::SessionError;

/// Aligné sur la barre de statut webview : cumuls ↑↓ et dernier « ctx »
/// (taille de prompt estimée côté provider ou jetons après snip/compaction).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUiStats {
    #[serde(default)]
    pub total_in: u64,
    #[serde(default)]
    pub total_out: u64,
    #[serde(default)]
    pub ctx: u32,
}

/// Lit le fichier s'il existe et est valide ; sinon `None`.
pub async fn read_session_ui_stats(path: &Utf8Path) -> Option<SessionUiStats> {
    let bytes = tokio::fs::read(path.as_std_path()).await.ok()?;
    serde_json::from_slice::<SessionUiStats>(&bytes).ok()
}

/// Écriture atomique (fichier temporaire + rename).
pub async fn write_session_ui_stats(path: &Utf8Path, stats: &SessionUiStats) -> Result<(), SessionError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent.as_std_path()).await?;
    }
    let tmp = camino::Utf8PathBuf::from(format!("{path}.tmp"));
    let data = serde_json::to_vec(stats)?;
    tokio::fs::write(tmp.as_std_path(), &data).await?;
    tokio::fs::rename(tmp.as_std_path(), path.as_std_path()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn roundtrip_ui_stats() {
        let dir = tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(dir.path().join("x.ui-stats.json")).unwrap();
        let s = SessionUiStats {
            total_in: 100,
            total_out: 20,
            ctx: 42,
        };
        write_session_ui_stats(&p, &s).await.unwrap();
        let got = read_session_ui_stats(&p).await.unwrap();
        assert_eq!(got, s);
    }
}
