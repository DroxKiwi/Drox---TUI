//! Navigation répertoires pour le modal `/workspace`.

use std::fs;
use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};

/// Racine « postes » (lecteurs) ou dossier concret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseLocation {
    /// Liste des volumes (Windows) ou `/` (Unix).
    Roots,
    Dir(Utf8PathBuf),
}

/// Entrée affichable dans l'explorateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseEntryKind {
    /// Remonter (`..` ou retour aux lecteurs).
    Parent,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseEntry {
    pub label: String,
    pub kind: BrowseEntryKind,
    /// Cible navigation (absent pour Parent depuis Roots).
    pub path: Option<Utf8PathBuf>,
}

impl BrowseLocation {
    #[must_use]
    pub fn display_path(&self) -> String {
        match self {
            Self::Roots => "Ordinateur".into(),
            Self::Dir(p) => p.to_string(),
        }
    }

    /// Point de départ : dossier courant du workspace ou home.
    #[must_use]
    pub fn from_hint(path: &str) -> Self {
        if path.trim().is_empty() {
            return Self::from_hint(&default_start_dir());
        }
        if let Ok(canonical) = fs::canonicalize(path) {
            if let Ok(u) = Utf8PathBuf::from_path_buf(canonical) {
                if u.as_std_path().is_dir() {
                    return Self::Dir(u);
                }
            }
        }
        if let Some(home) = dirs::home_dir().and_then(|h| Utf8PathBuf::from_path_buf(h).ok()) {
            return Self::Dir(home);
        }
        Self::Roots
    }

    #[must_use]
    pub fn selected_dir(&self) -> Option<&Utf8Path> {
        match self {
            Self::Roots => None,
            Self::Dir(p) => Some(p),
        }
    }

    pub fn enter(&mut self, entry: &BrowseEntry) -> bool {
        match entry.kind {
            BrowseEntryKind::Parent => {
                *self = self.parent_location();
                true
            }
            BrowseEntryKind::Directory => {
                if let Some(ref p) = entry.path {
                    *self = Self::Dir(p.clone());
                    true
                } else {
                    false
                }
            }
        }
    }

    #[must_use]
    pub fn parent_location(&self) -> Self {
        match self {
            Self::Roots => Self::Roots,
            Self::Dir(p) => {
                if p.parent().is_none() || is_volume_root(p) {
                    Self::Roots
                } else if let Some(parent) = p.parent() {
                    Self::Dir(parent.to_path_buf())
                } else {
                    Self::Roots
                }
            }
        }
    }
}

#[must_use]
pub fn list_location(loc: &BrowseLocation) -> Vec<BrowseEntry> {
    match loc {
        BrowseLocation::Roots => list_roots(),
        BrowseLocation::Dir(dir) => list_directory(dir),
    }
}

fn list_roots() -> Vec<BrowseEntry> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let path = PathBuf::from(&drive);
            if path.exists() {
                if let Ok(u) = Utf8PathBuf::from_path_buf(path) {
                    out.push(BrowseEntry {
                        label: drive,
                        kind: BrowseEntryKind::Directory,
                        path: Some(u),
                    });
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        out.push(BrowseEntry {
            label: "/".into(),
            kind: BrowseEntryKind::Directory,
            path: Some(Utf8PathBuf::from("/")),
        });
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

fn list_directory(dir: &Utf8Path) -> Vec<BrowseEntry> {
    let mut out = Vec::new();
    out.push(BrowseEntry {
        label: if is_volume_root(dir) {
            ".. Ordinateur".into()
        } else {
            "..".into()
        },
        kind: BrowseEntryKind::Parent,
        path: None,
    });

    let Ok(read) = fs::read_dir(dir.as_std_path()) else {
        return out;
    };

    let mut dirs = Vec::new();
    for entry in read.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let full = dir.join(&name);
        dirs.push(BrowseEntry {
            label: name,
            kind: BrowseEntryKind::Directory,
            path: Some(full),
        });
    }
    dirs.sort_by(|a, b| a.label.to_ascii_lowercase().cmp(&b.label.to_ascii_lowercase()));
    out.extend(dirs);
    out
}

fn is_volume_root(path: &Utf8Path) -> bool {
    let s = path.as_str();
    #[cfg(windows)]
    {
        s.len() == 3 && s.as_bytes()[1] == b':' && s.ends_with('\\')
    }
    #[cfg(not(windows))]
    {
        s == "/"
    }
}

fn default_start_dir() -> String {
    dirs::home_dir()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            #[cfg(windows)]
            {
                "C:\\".into()
            }
            #[cfg(not(windows))]
            {
                "/".into()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_project_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("readme.txt"), "x").unwrap();
        let loc = BrowseLocation::Dir(Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap());
        let entries = list_location(&loc);
        assert!(entries.first().is_some_and(|e| e.kind == BrowseEntryKind::Parent));
        assert!(entries.iter().any(|e| e.label == "src"));
        assert!(!entries.iter().any(|e| e.label == "readme.txt"));
    }
}
