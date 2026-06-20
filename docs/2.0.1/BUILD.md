# Compilation et installation — Drox TUI `2.0.1`

Produire un exécutable **release** pour distribution Windows ou Linux.

---

## 1. Prérequis

### Rust

```sh
# Installer rustup : https://rustup.rs/
cd drox
rustup show   # doit respecter rust-toolchain.toml (>= 1.85)
```

### Windows

- **Visual Studio Build Tools 2022** (ou VS Community) avec **« Développement Desktop en C++ »**
- Terminal : **Windows Terminal** recommandé
- Optionnel : **Ollama** pour l''inférence locale

### Linux (Debian/Ubuntu)

```sh
sudo apt update
sudo apt install build-essential pkg-config libssl-dev
```

Fedora/RHEL : `dnf groupinstall "Development Tools"` + `openssl-devel`.

---

## 2. Build release

```sh
git clone https://github.com/DroxKiwi/Drox---TUI.git
cd Drox---TUI/drox
cargo build --release -p drox-tui
```

Durée typique : 5–15 min (première compilation, selon machine).

| Mode | Commande | Sortie |
|---|---|---|
| **Release** | `cargo build --release -p drox-tui` | Optimisé, pour installation |
| **Debug** | `cargo build -p drox-tui` | `target/debug/`, développement |

### Emplacement du binaire

| OS | Chemin |
|---|---|
| Windows | `drox\target\release\drox-tui.exe` |
| Linux | `drox/target/release/drox-tui` |
| macOS | `drox/target/release/drox-tui` |

---

## 3. Installation

### Windows

```powershell
$dest = "$env:USERPROFILE\bin"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item "drox\target\release\drox-tui.exe" "$dest\drox-tui.exe"
# Ajouter %USERPROFILE%\bin au PATH utilisateur
drox-tui --list-sessions
```

Lancement :

```powershell
cd C:\Users\vous\projets\mon-repo
drox-tui --workspace .
```

### Linux

**Utilisateur** (`~/.local/bin`) :

```sh
install -m 755 drox/target/release/drox-tui ~/.local/bin/drox-tui
drox-tui --workspace ~/projets/mon-repo
```

**Système** :

```sh
sudo install -m 755 drox/target/release/drox-tui /usr/local/bin/drox-tui
```

---

## 4. Cross-compilation (optionnel)

| Scénario | Méthode |
|---|---|
| Windows → Windows | Build natif (section 2) |
| Linux → Linux | Build natif (section 2) |
| Windows → Linux | CI Linux, VM, ou [cross](https://github.com/cross-rs/cross) + Docker |
| Linux statique (musl) | `rustup target add x86_64-unknown-linux-musl` puis `cargo build --release -p drox-tui --target x86_64-unknown-linux-musl` |

Vérifier les dépendances dynamiques (Linux) :

```sh
ldd drox/target/release/drox-tui
```

---

## 5. Vérification

```sh
cargo test -p drox-tui          # tests unitaires
drox-tui --list-sessions        # smoke test CLI
drox-tui --workspace . -v       # logs → ~/.drox/tui.log
```

---

## 6. Confidentialité réseau

- **Aucune** télémétrie Drox dans le binaire.
- Connexion réseau **uniquement** vers le serveur LLM que vous configurez (`/server`, `DROX_SERVER`).
- Outils `web_fetch` / `web_search` / MCP : réseau **uniquement** si l''agent les appelle et si les permissions l''autorisent.
- Configuration **100 % hors-ligne** : Ollama local + règles `--deny` pour les outils web dans `~/.drox/settings.json`.