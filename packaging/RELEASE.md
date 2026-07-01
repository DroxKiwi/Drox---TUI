# Publication release Drox TUI (Windows + Linux)

Guide opérateur pour publier une version vers le dépôt **[Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR)** et créer la **GitHub Release**.

**Dépôt sources** : `Drox---TUI` (branche version, ex. `2.0.5`)  
**Dépôt OR** : `../Drox---TUI---OR` (binaires + README public)  
**Version** : lue depuis `drox/Cargo.toml` → `[workspace.package] version`

---

## Prérequis

| Plateforme | Outils |
|---|---|
| **Windows** | Rust ≥ 1.85, PowerShell 5.1+, [Inno Setup 6](https://jrsoftware.org/isdl.php) (auto via script) |
| **Linux (WSL ou natif)** | Rust ≥ 1.85, `tar`, `sha256sum` |
| **Commun** | `git`, [`gh`](https://cli.github.com/) CLI authentifié |

Sur Windows, le build Linux passe par **WSL** (Ubuntu recommandé) :

```powershell
wsl -d Ubuntu bash -lc "source ~/.cargo/env && rustc --version"
# Si absent : curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Les scripts `packaging/**/*.sh` sont en **LF** (`.gitattributes`). Si WSL échoue avec `bash\r`, exécuter une fois :

```bash
sed -i 's/\r$//' packaging/build-and-pack-linux.sh packaging/linux/install.sh
```

---

## Vue d'ensemble

```text
drox/Cargo.toml (version)
        │
        ├─► build-and-pack.ps1          → dist/drox-tui-<ver>-windows-x64-setup.exe
        ├─► build-and-pack-linux.sh    → dist/drox-tui-<ver>-linux-x64.tar.gz
        │
        ├─► publish-or.ps1             → copie OR + latest.json + commit OR
        └─► publish-linux-or.ps1       → Linux seul (complément release existante)
                │
                ▼
        Drox---TUI---OR/releases/v<ver>/
                │
                ▼
        gh release create / upload  →  GitHub Releases (assets téléchargeables)
```

---

## 1. Préparer la version (sources)

1. Bumper `version` dans `drox/Cargo.toml` (workspace).
2. Rédiger `docs/<version>/RELEASE_NOTES.md`.
3. Mettre à jour `README.md` (version courante, liens installateur).
4. Commit + push sur la branche version :

```powershell
cd C:\Users\coren\Desktop\GitHub\Drox---TUI
git checkout 2.0.5   # exemple
git add ...
git commit -m "feat(2.0.5): ..."
git push -u origin 2.0.5
```

---

## 2. Release Windows

### Build seul (test local)

```powershell
.\packaging\build-and-pack.ps1
# Artefact : dist\drox-tui-<version>-windows-x64-setup.exe
# Empreinte : dist\SHA256SUMS-<version>-windows.txt
```

### Publier vers OR (Windows seul)

```powershell
.\packaging\publish-or.ps1 -SkipLinux
```

Copie vers `Drox---TUI---OR/releases/v<version>/` :

- `drox-tui-<version>-windows-x64-setup.exe`
- `SHA256SUMS-windows.txt`
- `RELEASE_NOTES.md` (+ empreinte Windows)
- `releases/latest.json` (clé `windows_x64`)
- `README.md` public

Commit OR automatique : `release: Drox TUI v<version> (Windows x64)`.

---

## 3. Release Linux

### Build seul (WSL depuis Windows)

```powershell
wsl -d Ubuntu bash -lc "cd /mnt/c/Users/coren/Desktop/GitHub/Drox---TUI && source ~/.cargo/env && bash packaging/build-and-pack-linux.sh"
```

Ou sur machine Linux :

```bash
./packaging/build-and-pack-linux.sh
```

Artefact : `dist/drox-tui-<version>-linux-x64.tar.gz`

Contenu de l'archive :

```text
drox-tui-<version>-linux-x64/
├── drox-tui          # binaire
├── install.sh        # ~/.local/bin ou --system
├── README-INSTALL.txt
├── LICENSE
└── VERSION
```

### Publier Linux vers OR (release Windows déjà faite)

```powershell
.\packaging\publish-linux-or.ps1
# ou si l'archive est déjà dans dist/ :
.\packaging\publish-linux-or.ps1 -SkipBuild
```

Ajoute dans `Drox---TUI---OR/releases/v<version>/` :

- `drox-tui-<version>-linux-x64.tar.gz`
- `SHA256SUMS-linux.txt`
- Met à jour `releases/latest.json` (clé `linux_x64`)

### Windows + Linux en une commande

```powershell
.\packaging\publish-or.ps1
# sans -SkipLinux : build WSL + Windows, puis copie complète OR
```

---

## 4. GitHub Release (OR)

Après commit OR, pousser :

```powershell
cd ..\Drox---TUI---OR
git push origin main
```

### Créer une nouvelle release (première publication de la version)

```powershell
cd ..\Drox---TUI---OR
gh release create v2.0.5 `
  --title "Drox TUI 2.0.5" `
  --notes-file "releases/v2.0.5/RELEASE_NOTES.md" `
  "releases/v2.0.5/drox-tui-2.0.5-windows-x64-setup.exe"
```

### Ajouter l'asset Linux à une release existante

```powershell
gh release upload v2.0.5 "releases/v2.0.5/drox-tui-2.0.5-linux-x64.tar.gz"
```

Vérifier : https://github.com/DroxKiwi/Drox---TUI---OR/releases

---

## 5. Vérifications post-release

| Check | Commande / action |
|---|---|
| Installateur Windows | Nouveau terminal → `drox-tui --version` |
| Archive Linux | `tar xzf ... && ./install.sh && drox-tui --version` |
| `/update` | Client 2.0.4 avec `/update on` → détecte `latest.json` |
| SHA256 | Comparer `SHA256SUMS-*.txt` et empreintes dans RELEASE_NOTES |

---

## 6. Structure dépôt OR

```text
Drox---TUI---OR/
├── README.md                    # copié depuis sources à chaque publish-or
├── install/
│   ├── windows/install.ps1
│   └── linux/install.sh
├── releases/
│   ├── latest.json              # /update (windows_x64, linux_x64)
│   └── v2.0.5/
│       ├── drox-tui-2.0.5-windows-x64-setup.exe
│       ├── drox-tui-2.0.5-linux-x64.tar.gz
│       ├── SHA256SUMS-windows.txt
│       ├── SHA256SUMS-linux.txt
│       └── RELEASE_NOTES.md
```

---

## Dépannage

| Problème | Solution |
|---|---|
| `bash\r: No such file` (WSL) | CRLF → `sed -i 's/\r$//' packaging/*.sh` ou checkout LF |
| `cargo: command not found` (WSL) | `curl … \| sh` rustup, puis `source ~/.cargo/env` |
| Inno Setup absent | `winget install JRSoftware.InnoSetup` ou relancer `build-and-pack.ps1` |
| OR introuvable | `-OrRepo "C:\chemin\Drox---TUI---OR"` |
| Release GH sans Linux | `gh release upload v<ver> releases/v<ver>/*.tar.gz` |

---

## Scripts référence

| Script | Rôle |
|---|---|
| `packaging/build-and-pack.ps1` | Build release + installateur Windows |
| `packaging/build-and-pack-linux.sh` | Build release + tar.gz Linux |
| `packaging/publish-or.ps1` | Windows (+ Linux optionnel) → OR |
| `packaging/publish-linux-or.ps1` | Linux seul → OR (complément) |

Voir aussi [`packaging/README.md`](README.md) (résumé) et [`README.dev.md`](../README.dev.md).
