# Publication release Drox TUI (Windows + Linux)

Guide opérateur pour publier une version et créer la **GitHub Release** sur **ce** dépôt ([Drox---TUI](https://github.com/DroxKiwi/Drox---TUI)).

**Dépôt sources** : `Drox---TUI` (`main`)  
**Version** : lue depuis `drox/Cargo.toml` → `[workspace.package] version`

---

## Prérequis

| Plateforme | Outils |
|---|---|
| **Windows** | Rust ≥ 1.85, PowerShell 5.1+, [Inno Setup 6](https://jrsoftware.org/isdl.php) |
| **Linux (WSL ou natif)** | Rust ≥ 1.85, `tar`, `sha256sum` |
| **Commun** | `git`, [`gh`](https://cli.github.com/) CLI authentifié |

Sur Windows, le build Linux passe par **WSL** (Ubuntu recommandé).

---

## Vue d'ensemble

```text
drox/Cargo.toml (version)
        │
        ├─► packaging/build-and-pack.ps1       → dist/drox-tui-<ver>-windows-x64-setup.exe
        ├─► packaging/build-and-pack-linux.sh  → dist/drox-tui-<ver>-linux-x64.tar.gz
        │
        ├─► releases/latest.json               → /update (opt-in)
        └─► gh release create / upload         → GitHub Releases (ce dépôt)
```

Les anciens scripts `publish-or.ps1` / `publish-linux-or.ps1` ciblaient le miroir OR — le canal officiel est désormais **Drox---TUI**.

---

## Build

```powershell
# Windows
.\packaging\build-and-pack.ps1

# Linux (depuis Windows via WSL)
wsl -d Ubuntu bash -lc "cd /mnt/c/Users/coren/Desktop/GitHub/Drox---TUI && source ~/.cargo/env && bash packaging/build-and-pack-linux.sh"
```

## Publier sur GitHub Releases

```powershell
# Exemple 2.0.5
gh release create v2.0.5 `
  --repo DroxKiwi/Drox---TUI `
  --title "Drox TUI 2.0.5" `
  --notes-file "docs/RELEASE_NOTES.md" `
  "dist/drox-tui-2.0.5-windows-x64-setup.exe" `
  "dist/drox-tui-2.0.5-linux-x64.tar.gz"
```

Mettre à jour `releases/latest.json` (URLs `https://github.com/DroxKiwi/Drox---TUI/releases/download/...`).

---

## Scripts référence

| Script | Rôle |
|---|---|
| `packaging/build-and-pack.ps1` | Build release + installateur Windows |
| `packaging/build-and-pack-linux.sh` | Build release + tar.gz Linux |
| `packaging/publish-or.ps1` | Legacy OR (déconseillé) |
| `packaging/publish-linux-or.ps1` | Legacy OR Linux (déconseillé) |
