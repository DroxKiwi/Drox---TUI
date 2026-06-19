//! Collage d'images — chemins fichier et presse-papiers (leak : `imagePaste.ts`).
//!
//! Le composer affiche `[Image #N]` ; à l'envoi, les blocs `Content::Image` sont
//! passés au moteur (`run_with_history_blocks`).

use std::path::Path;
use std::process::Command;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use camino::{Utf8Path, Utf8PathBuf};

/// Extensions image supportées (aligné leak `IMAGE_EXTENSION_REGEX`).
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Taille max lue depuis le disque / clipboard (évite de saturer le contexte).
pub const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;

/// Données image prêtes pour le store paste.
#[derive(Debug, Clone)]
pub struct StoredImage {
    pub mime: String,
    pub data_base64: String,
    /// Libellé affiché dans les notices système (chemin ou « presse-papiers »).
    pub label: String,
    pub byte_len: usize,
}

/// Référence affichée dans le composer (`history.ts` : `formatImageRef`).
#[must_use]
pub fn format_image_ref(id: u32) -> String {
    format!("[Image #{id}]")
}

/// Parse les références `[Image #N]` présentes dans le texte (ordre d'apparition).
#[must_use]
pub fn parse_image_refs(input: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = input[search..].find("[Image #") {
        let idx = search + rel;
        let rest = &input[idx + "[Image #".len()..];
        let id_end = rest.find(|c: char| !c.is_ascii_digit());
        let Some(id_end) = id_end else {
            break;
        };
        if let Ok(id) = rest[..id_end].parse::<u32>() {
            if rest[id_end..].starts_with(']') {
                out.push(id);
            }
        }
        search = idx + 1;
    }
    out
}

/// Indique si le texte collé ressemble à un chemin d'image.
#[must_use]
#[allow(dead_code)] // API publique alignée leak ; utilisée dans les tests.
pub fn is_image_file_path(text: &str) -> bool {
    as_image_file_path(text).is_some()
}

/// Normalise un chemin image collé (guillemets, espaces).
#[must_use]
pub fn as_image_file_path(text: &str) -> Option<String> {
    let cleaned = remove_outer_quotes(text.trim());
    let ext = Path::new(&cleaned)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some(cleaned)
    } else {
        None
    }
}

fn remove_outer_quotes(text: &str) -> String {
    if (text.starts_with('"') && text.ends_with('"')) || (text.starts_with('\'') && text.ends_with('\'')) {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

fn mime_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Lit une image depuis un chemin (absolu ou relatif aux racines workspace).
#[must_use]
pub fn read_image_file(path: &str, roots: &[Utf8PathBuf]) -> Option<StoredImage> {
    let candidates = resolve_image_path(path, roots);
    for candidate in candidates {
        if let Some(img) = read_bytes_at(&candidate) {
            return Some(img);
        }
    }
    None
}

fn resolve_image_path(path: &str, roots: &[Utf8PathBuf]) -> Vec<Utf8PathBuf> {
    let mut out = Vec::new();
    if Utf8Path::new(path).is_absolute() {
        out.push(Utf8PathBuf::from(path));
    } else {
        for root in roots {
            out.push(root.join(path));
        }
        out.push(Utf8PathBuf::from(path));
    }
    out
}

fn read_bytes_at(path: &Utf8Path) -> Option<StoredImage> {
    let bytes = std::fs::read(path.as_std_path()).ok()?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let mime = mime_for_path(path.as_str()).to_string();
    let label = path.as_str().to_string();
    Some(StoredImage {
        mime,
        data_base64: STANDARD.encode(&bytes),
        label,
        byte_len: bytes.len(),
    })
}

/// Tente de lire une image depuis le presse-papiers système.
#[must_use]
pub fn read_clipboard_image() -> Option<StoredImage> {
    #[cfg(windows)]
    {
        return read_clipboard_image_windows();
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn read_clipboard_image_windows() -> Option<StoredImage> {
    let temp = std::env::temp_dir().join("drox_tui_clipboard.png");
    let path_str = temp.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        "$img = Get-Clipboard -Format Image; if ($img) {{ $img.Save('{path_str}', [System.Drawing.Imaging.ImageFormat]::Png) }}"
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let bytes = std::fs::read(&temp).ok()?;
    let _ = std::fs::remove_file(&temp);
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    Some(StoredImage {
        mime: "image/png".into(),
        data_base64: STANDARD.encode(&bytes),
        label: "presse-papiers".into(),
        byte_len: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_image_extension() {
        assert!(is_image_file_path(r#"C:\tmp\shot.png"#));
        assert!(is_image_file_path("\"/home/u/pic.JPEG\""));
        assert!(!is_image_file_path("readme.md"));
    }

    #[test]
    fn parse_image_ref_ids() {
        let ids = parse_image_refs("voir [Image #2] et [Image #5]");
        assert_eq!(ids, vec![2, 5]);
    }

    #[test]
    fn format_ref() {
        assert_eq!(format_image_ref(3), "[Image #3]");
    }
}
