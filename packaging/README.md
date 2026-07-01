# Pipeline release officielle — Drox TUI

> **Guide complet** : [`RELEASE.md`](RELEASE.md) (Windows + Linux, OR, GitHub Release).

Produit **2.0.5** · moteur dérivé IDE **1.5.0** · dépôt OR : `../Drox---TUI---OR`

## Prérequis Windows (build installateur)

- Rust ≥ 1.85
- [Inno Setup 6](https://jrsoftware.org/isdl.php) — ou `winget install JRSoftware.InnoSetup` (auto via le script)

## Prérequis Linux (WSL ou machine Linux)

- Rust ≥ 1.85 (`rustup` dans WSL Ubuntu)
- Scripts shell en LF (voir `.gitattributes`)

## Windows

```powershell
# Build + installateur .exe
.\packaging\build-and-pack.ps1

# Build + publier OR (Windows seul)
.\packaging\publish-or.ps1 -SkipLinux

# Windows + Linux (WSL)
.\packaging\publish-or.ps1
```

Artefact : `dist/drox-tui-<version>-windows-x64-setup.exe`

## Linux

```powershell
# Build WSL + publier OR (complément d'une release Windows existante)
.\packaging\publish-linux-or.ps1

# Ou sur Linux natif :
./packaging/build-and-pack-linux.sh
```

Artefact : `dist/drox-tui-<version>-linux-x64.tar.gz`

## GitHub Release

```powershell
cd ..\Drox---TUI---OR
git push origin main
gh release upload v2.0.5 "releases/v2.0.5/drox-tui-2.0.5-linux-x64.tar.gz"
```

## Structure dépôt OR

```text
Drox---TUI---OR/
├── README.md
├── install/
│   ├── windows/install.ps1
│   └── linux/install.sh
└── releases/
    ├── latest.json
    └── v2.0.5/
        ├── drox-tui-2.0.5-windows-x64-setup.exe
        ├── drox-tui-2.0.5-linux-x64.tar.gz
        ├── SHA256SUMS-windows.txt
        ├── SHA256SUMS-linux.txt
        └── RELEASE_NOTES.md
```
