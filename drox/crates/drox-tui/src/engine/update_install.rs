//! Téléchargement, vérification SHA256 et installation MAJ TUI.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
#[cfg(target_os = "linux")]
use walkdir::WalkDir;

use super::update::{LatestRelease, PlatformAsset, LOCAL_VERSION};

/// Artefact plateforme courante.
#[must_use]
pub fn platform_asset(release: &LatestRelease) -> Option<&PlatformAsset> {
    #[cfg(windows)]
    {
        return release.windows_x64.as_ref();
    }
    #[cfg(target_os = "linux")]
    {
        return release.linux_x64.as_ref();
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = release;
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    /// Installateur graphique lancé (Windows setup.exe).
    LaunchedInstaller { path: PathBuf },
    /// Script de remplacement + relance (binaire connu).
    ScheduledReplace { script: PathBuf, target: PathBuf },
    /// Artefact prêt — instructions manuelles.
    Manual { artifact: PathBuf, hint: String },
}

pub fn staging_dir(version: &str) -> anyhow::Result<PathBuf> {
    let base = std::env::temp_dir().join("drox-tui-update").join(version);
    std::fs::create_dir_all(&base).with_context(|| format!("création {}", base.display()))?;
    Ok(base)
}

pub fn file_name_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

pub fn verify_file_sha256(path: &Path, expected: &str) -> anyhow::Result<()> {
    let expected = expected.trim().to_ascii_lowercase();
    let bytes = std::fs::read(path).with_context(|| format!("lecture {}", path.display()))?;
    let got = format!("{:x}", Sha256::digest(bytes));
    if got != expected {
        anyhow::bail!("SHA256 invalide — attendu {expected}, reçu {got}");
    }
    Ok(())
}

pub async fn download_asset(asset: &PlatformAsset, version: &str) -> anyhow::Result<PathBuf> {
    let dest = staging_dir(version)?.join(file_name_from_url(&asset.url));
    if dest.exists() {
        let _ = std::fs::remove_file(&dest);
    }

    let client = reqwest::Client::builder()
        .user_agent(format!("drox-tui/{LOCAL_VERSION} update-install"))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("client HTTP téléchargement")?;

    let resp = client
        .get(&asset.url)
        .send()
        .await
        .with_context(|| format!("GET {}", asset.url))?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} — {}", resp.status(), asset.url);
    }

    let bytes = resp.bytes().await.context("corps téléchargement")?;
    std::fs::write(&dest, &bytes).with_context(|| format!("écriture {}", dest.display()))?;
    verify_file_sha256(&dest, &asset.sha256)?;
    tracing::info!(path = %dest.display(), "MAJ artefact vérifié (SHA256)");
    Ok(dest)
}

/// Chemin du binaire installé (prefs ou exécutable courant).
#[must_use]
pub fn resolve_install_path(
    prefs_path: Option<&str>,
) -> Option<PathBuf> {
    if let Some(p) = prefs_path {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    std::env::current_exe()
        .ok()
        .filter(|p| p.is_file())
}

#[must_use]
pub fn default_windows_install_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("AppData").join("Local").join("Programs").join("DroxTUI").join("bin").join("drox-tui.exe"))
        .unwrap_or_else(|| PathBuf::from(r"%LOCALAPPDATA%\Programs\DroxTUI\bin\drox-tui.exe"))
}

fn is_windows_setup(name: &str) -> bool {
    name.to_ascii_lowercase().contains("setup")
}

#[cfg(target_os = "linux")]
fn extract_tar_gz(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .context("lancement tar")?;
    if !status.success() {
        anyhow::bail!("tar a échoué ({status})");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn find_linux_binary(root: &Path) -> Option<PathBuf> {
    for ent in WalkDir::new(root).max_depth(4).into_iter().flatten() {
        if ent.file_type().is_file() {
            let name = ent.file_name().to_string_lossy();
            if name == "drox-tui" {
                return Some(ent.path().to_path_buf());
            }
        }
    }
    None
}

#[cfg(windows)]
fn write_windows_replace_script(
    source: &Path,
    target: &Path,
    pid: u32,
    args: &[String],
) -> anyhow::Result<PathBuf> {
    let script_path = staging_dir("replace")?.join("drox-tui-update.ps1");
    let args_joined = args
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");
    let body = format!(
        r#"$ErrorActionPreference = 'Stop'
$pidWait = {pid}
$src = '{}'
$dst = '{}'
while (Get-Process -Id $pidWait -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 400 }}
Copy-Item -LiteralPath $src -Destination $dst -Force
Start-Process -FilePath $dst -ArgumentList @({args_joined})
"#,
        source.display(),
        target.display(),
    );
    std::fs::write(&script_path, body)?;
    Ok(script_path)
}

#[cfg(not(windows))]
fn write_windows_replace_script(
    _source: &Path,
    _target: &Path,
    _pid: u32,
    _args: &[String],
) -> anyhow::Result<PathBuf> {
    anyhow::bail!("remplacement Windows indisponible sur cette plateforme")
}

#[cfg(unix)]
fn write_unix_replace_script(
    source: &Path,
    target: &Path,
    pid: u32,
    args: &[String],
) -> anyhow::Result<PathBuf> {
    let script_path = staging_dir("replace")?.join("drox-tui-update.sh");
    let args_quoted = args
        .iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    let body = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nwhile kill -0 {pid} 2>/dev/null; do sleep 0.4; done\ninstall -m 755 '{src}' '{dst}'\nexec '{dst}' {args_quoted}\n",
        src = source.display(),
        dst = target.display(),
    );
    std::fs::write(&script_path, &body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(script_path)
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn write_unix_replace_script(
    _source: &Path,
    _target: &Path,
    _pid: u32,
    _args: &[String],
) -> anyhow::Result<PathBuf> {
    anyhow::bail!("remplacement Unix indisponible sur cette plateforme")
}

#[cfg(unix)]
fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':')
    }) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

fn spawn_detached(script: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                script.to_str().context("chemin script")?,
            ])
            .spawn()
            .context("lancement script MAJ")?;
    }
    #[cfg(unix)]
    {
        std::process::Command::new(script)
            .spawn()
            .context("lancement script MAJ")?;
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = script;
        anyhow::bail!("plateforme non supportée pour relance");
    }
    Ok(())
}

pub fn collect_process_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

pub async fn apply_install(
    release: &LatestRelease,
    asset: &PlatformAsset,
    prefs_install_path: Option<&str>,
) -> anyhow::Result<InstallOutcome> {
    let artifact = download_asset(asset, &release.version).await?;
    let name = artifact
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    #[cfg(windows)]
    {
        if is_windows_setup(name) {
            Command::new(&artifact)
                .spawn()
                .with_context(|| format!("lancement installateur {}", artifact.display()))?;
            return Ok(InstallOutcome::LaunchedInstaller {
                path: artifact,
            });
        }
        let target = resolve_install_path(prefs_install_path)
            .or_else(|| {
                let def = default_windows_install_path();
                if def.is_file() {
                    Some(def)
                } else {
                    None
                }
            });
        if let Some(target) = target {
            let script = write_windows_replace_script(
                &artifact,
                &target,
                std::process::id(),
                &collect_process_args(),
            )?;
            spawn_detached(&script)?;
            return Ok(InstallOutcome::ScheduledReplace {
                script,
                target,
            });
        }
        return Ok(InstallOutcome::Manual {
            artifact,
            hint: "Lancez l'installateur téléchargé ou copiez drox-tui.exe manuellement.".into(),
        });
    }

    #[cfg(target_os = "linux")]
    {
        let extract_root = staging_dir(&format!("{}-extract", release.version))?;
        extract_tar_gz(&artifact, &extract_root)?;
        let bin = find_linux_binary(&extract_root)
            .with_context(|| "binaire drox-tui introuvable dans l'archive")?;
        let target = resolve_install_path(prefs_install_path).or_else(|| {
            dirs::home_dir().map(|h| h.join(".local").join("bin").join("drox-tui"))
        });
        if let Some(target) = target {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let script = write_unix_replace_script(
                &bin,
                &target,
                std::process::id(),
                &collect_process_args(),
            )?;
            spawn_detached(&script)?;
            return Ok(InstallOutcome::ScheduledReplace {
                script,
                target,
            });
        }
        return Ok(InstallOutcome::Manual {
            artifact: bin,
            hint: format!(
                "Exécutez : install -m 755 '{}' ~/.local/bin/drox-tui",
                bin.display()
            ),
        });
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (release, asset, prefs_install_path, artifact, name);
        anyhow::bail!("installation automatique non supportée sur cette plateforme")
    }
}

pub fn persist_install_path(path: &Path) -> anyhow::Result<()> {
    let Some(s) = path.to_str() else {
        return Ok(());
    };
    let mut prefs = crate::engine::preferences::load_preferences();
    prefs.update.install_path = Some(s.to_string());
    crate::engine::preferences::save_preferences(&prefs)
}

#[must_use]
pub fn install_path_utf8(path: &Path) -> Option<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn verify_sha256_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("t.bin");
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(b"hello-drox").unwrap();
        let hash = "2dbda0e76c6fa7fef812ed6df9860a609a53b219b050bd519bf1d3feec3ffc18";
        verify_file_sha256(&file, hash).expect("hash ok");
    }

    #[test]
    fn detects_setup_name() {
        assert!(is_windows_setup("drox-tui-2.0.3-windows-x64-setup.exe"));
        assert!(!is_windows_setup("drox-tui.exe"));
    }

    #[test]
    fn file_name_from_url_parses() {
        assert_eq!(
            file_name_from_url("https://x.com/v1/drox-tui-2.0.3-setup.exe"),
            "drox-tui-2.0.3-setup.exe"
        );
    }
}
