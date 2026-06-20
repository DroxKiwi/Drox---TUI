# Pipeline release officielle — Drox TUI

Produit **2.0.1** · moteur dérivé IDE **1.5.0** · dépôt OR : `../Drox---TUI---OR`

## Prérequis Windows (build installateur)

- Rust ≥ 1.85
- [Inno Setup 6](https://jrsoftware.org/isdl.php) — ou `winget install JRSoftware.InnoSetup` (auto via le script)

## Windows (depuis ce repo)

```powershell
# Build + installateur .exe
.\packaging\build-and-pack.ps1

# Build + installateur + copie vers Drox---TUI---OR (+ commit git OR)
.\packaging\publish-or.ps1
```

Artefact : `dist/drox-tui-<version>-windows-x64-setup.exe`  
Script Inno Setup : `packaging/windows/drox-tui-setup.iss`

### Installation utilisateur

Double-clic sur `drox-tui-2.0.1-windows-x64-setup.exe`, puis :

```powershell
drox-tui --workspace C:\chemin\projet
```

Installe dans `%LOCALAPPDATA%\Programs\DroxTUI\bin`, PATH utilisateur (option), entrée Désinstaller dans Windows.

## Linux (machine Linux ou WSL)

```bash
chmod +x packaging/build-and-pack-linux.sh
./packaging/build-and-pack-linux.sh
```

Artefact : `dist/drox-tui-<version>-linux-x64.tar.gz`

## Structure dépôt OR

```text
Drox---TUI---OR/
├── README.md
├── install/
│   ├── windows/install.ps1   # installation manuelle (optionnel)
│   └── linux/install.sh
└── releases/
    └── v2.0.1/
        ├── drox-tui-2.0.1-windows-x64-setup.exe
        ├── SHA256SUMS-windows.txt
        ├── drox-tui-2.0.1-linux-x64.tar.gz
        └── RELEASE_NOTES.md
```
